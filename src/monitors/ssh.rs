//! SSH authentication log monitor.
//!
//! On Linux, follows journald units `ssh` / `sshd` (and `sshd-session` on newer Ubuntu)
//! via `journalctl`. Parsing is platform-agnostic so unit tests can run without journald.

use std::time::Duration;

use crate::{error_fmt, info, warn, warn_fmt};
use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::sleep;

#[cfg(target_os = "linux")]
use crate::debug;

#[cfg(target_os = "linux")]
use std::process::Stdio;
#[cfg(target_os = "linux")]
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(target_os = "linux")]
use tokio::process::{Child, Command};

use crate::error::{AgentError, Result};
use crate::events::{EventType, SecurityEvent, Severity};

const RECONNECT_DELAY: Duration = Duration::from_secs(3);

/// Parsed SSH authentication attempt extracted from a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshAuthEvent {
    pub event_type: EventType,
    pub username: Option<String>,
    pub source_ip: Option<String>,
    pub auth_method: Option<String>,
    pub raw: String,
}

/// Parse a single sshd / sshd-session journal or syslog line.
pub fn parse_ssh_log_line(line: &str) -> Option<SshAuthEvent> {
    let message = extract_message(line);
    parse_ssh_message(&message)
}

fn extract_message(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.starts_with('{')
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(msg) = value
            .get("MESSAGE")
            .or_else(|| value.get("message"))
            .and_then(|v| v.as_str())
    {
        return msg.to_string();
    }
    // Strip common syslog prefix: "MMM DD HH:MM:SS host sshd[pid]: "
    if let Some(idx) = trimmed.find(": ") {
        let after = &trimmed[idx + 2..];
        if trimmed.contains("sshd") || trimmed.contains("sshd-session") {
            return after.to_string();
        }
    }
    trimmed.to_string()
}

fn parse_ssh_message(message: &str) -> Option<SshAuthEvent> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }

    // Accepted publickey/password/keyboard-interactive for user from IP ...
    if let Some(rest) = message.strip_prefix("Accepted ") {
        return parse_accepted(rest, message);
    }

    // Failed password / Failed publickey for ...
    if let Some(rest) = message.strip_prefix("Failed ") {
        return parse_failed(rest, message);
    }

    // Invalid user admin from 1.2.3.4 port 22
    if let Some(rest) = message.strip_prefix("Invalid user ") {
        return parse_invalid_user(rest, message);
    }

    // Connection closed by authenticating user ... (auth failure variants)
    if message.contains("authentication failure")
        || message.contains("Failed none for")
        || message.starts_with("error: maximum authentication attempts exceeded")
    {
        let username =
            extract_token_after(message, "user ").or_else(|| extract_token_after(message, "for "));
        let source_ip = extract_ip(message);
        return Some(SshAuthEvent {
            event_type: EventType::SshAuthFailure,
            username: username.map(normalize_username),
            source_ip,
            auth_method: None,
            raw: message.to_string(),
        });
    }

    None
}

fn parse_accepted(rest: &str, raw: &str) -> Option<SshAuthEvent> {
    // publickey for alice from 203.0.113.10 port 22 ssh2: ...
    let auth_method = rest.split_whitespace().next().map(str::to_string);
    let username = extract_token_after(rest, "for ")?;
    let source_ip = extract_token_after(rest, "from ")?;
    let username = normalize_username(username);
    let event_type = if username == "root" {
        EventType::SshRootLogin
    } else {
        EventType::SshLoginSuccess
    };
    Some(SshAuthEvent {
        event_type,
        username: Some(username),
        source_ip: Some(source_ip.to_string()),
        auth_method,
        raw: raw.to_string(),
    })
}

fn parse_failed(rest: &str, raw: &str) -> Option<SshAuthEvent> {
    // password for invalid user admin from 1.2.3.4 port 22 ssh2
    // password for alice from 1.2.3.4 port 22 ssh2
    // publickey for root from 1.2.3.4 port 22 ssh2
    let auth_method = rest.split_whitespace().next().map(str::to_string);
    let after_for = remainder_after(rest, "for ")?;
    let (username, source_ip) = if let Some(rest_user) = after_for.strip_prefix("invalid user ") {
        let username = rest_user.split_whitespace().next()?;
        let source_ip = extract_token_after(rest_user, "from ")?;
        (username, source_ip)
    } else {
        let username = after_for.split_whitespace().next()?;
        let source_ip = extract_token_after(after_for, "from ")?;
        (username, source_ip)
    };

    Some(SshAuthEvent {
        event_type: EventType::SshLoginFailure,
        username: Some(normalize_username(username)),
        source_ip: Some(source_ip.to_string()),
        auth_method,
        raw: raw.to_string(),
    })
}

fn parse_invalid_user(rest: &str, raw: &str) -> Option<SshAuthEvent> {
    let username = rest.split_whitespace().next()?;
    let source_ip = extract_token_after(rest, "from ")?;
    Some(SshAuthEvent {
        event_type: EventType::SshAuthFailure,
        username: Some(normalize_username(username)),
        source_ip: Some(source_ip.to_string()),
        auth_method: None,
        raw: raw.to_string(),
    })
}

fn remainder_after<'a>(haystack: &'a str, marker: &str) -> Option<&'a str> {
    let idx = haystack.find(marker)?;
    Some(haystack[idx + marker.len()..].trim_start())
}

fn extract_token_after<'a>(haystack: &'a str, marker: &str) -> Option<&'a str> {
    let rest = remainder_after(haystack, marker)?;
    let token = rest.split_whitespace().next()?;
    // Strip trailing punctuation sometimes present in logs
    Some(token.trim_matches(|c: char| c == ',' || c == ';' || c == ':'))
}

fn extract_ip(message: &str) -> Option<String> {
    extract_token_after(message, "from ").map(str::to_string)
}

fn normalize_username(username: &str) -> String {
    username.trim().to_string()
}

impl SshAuthEvent {
    pub fn into_security_event(self, agent_id: &str, timestamp: DateTime<Utc>) -> SecurityEvent {
        let severity = match self.event_type {
            EventType::SshRootLogin => Severity::High,
            EventType::SshLoginFailure | EventType::SshAuthFailure => Severity::Medium,
            EventType::SshLoginSuccess => Severity::Info,
            _ => Severity::Info,
        };

        let mut event =
            SecurityEvent::new(agent_id, self.event_type, severity).with_timestamp(timestamp);
        if let Some(ip) = self.source_ip {
            event = event.with_source_ip(ip);
        }
        if let Some(user) = self.username {
            event = event.with_username(user);
        }
        let mut meta = json!({ "raw": self.raw });
        if let Some(method) = self.auth_method {
            meta["auth_method"] = json!(method);
        }
        event.with_metadata(meta)
    }
}

/// Follows SSH-related journal units and emits security events.
pub struct SshMonitor {
    agent_id: String,
    tx: mpsc::Sender<SecurityEvent>,
}

impl SshMonitor {
    pub fn new(agent_id: String, tx: mpsc::Sender<SecurityEvent>) -> Self {
        Self { agent_id, tx }
    }

    pub async fn run(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(SSH_MONITOR_STARTED);
        loop {
            if *shutdown.borrow() {
                break;
            }

            match follow_journal(&self.agent_id, &self.tx, &mut shutdown).await {
                Ok(()) => {
                    if *shutdown.borrow() {
                        break;
                    }
                    warn_fmt!(
                        SSH_JOURNAL_RECONNECT,
                        "delay" => format!("{RECONNECT_DELAY:?}")
                    );
                }
                Err(AgentError::PermissionDenied(msg)) => {
                    error_fmt!(SSH_PERMISSION_DENIED, "msg" => msg);
                    // Do not spin-restart on permission errors; wait for shutdown.
                    let _ = shutdown.changed().await;
                    break;
                }
                Err(err) => {
                    warn_fmt!(
                        SSH_MONITOR_ERROR,
                        "error" => err,
                        "delay" => format!("{RECONNECT_DELAY:?}")
                    );
                }
            }

            tokio::select! {
                _ = sleep(RECONNECT_DELAY) => {}
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
        info!(SSH_MONITOR_STOPPED);
    }
}

#[cfg(target_os = "linux")]
async fn follow_journal(
    agent_id: &str,
    tx: &mpsc::Sender<SecurityEvent>,
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let mut child = spawn_journalctl()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AgentError::LogSourceDisconnected("journalctl stdout missing".into()))?;
    let mut lines = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    let _ = child.kill().await;
                    return Ok(());
                }
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        handle_line(agent_id, tx, &line).await;
                    }
                    Ok(None) => {
                        let _ = child.wait().await;
                        return Err(AgentError::LogSourceDisconnected(
                            "journalctl stdout closed".into(),
                        ));
                    }
                    Err(err) => {
                        let _ = child.kill().await;
                        return Err(AgentError::Io(err));
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
async fn follow_journal(
    _agent_id: &str,
    _tx: &mpsc::Sender<SecurityEvent>,
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    warn!(SSH_LINUX_ONLY);
    let _ = shutdown.changed().await;
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn_journalctl() -> Result<Child> {
    // Cover classic sshd and Ubuntu 24.04+ sshd-session units.
    let mut cmd = Command::new("journalctl");
    cmd.args([
        "-f",
        "-n",
        "0",
        "-o",
        "json",
        "--no-pager",
        "-u",
        "ssh.service",
        "-u",
        "sshd.service",
        "-u",
        "ssh",
        "-u",
        "sshd",
        "_COMM=sshd",
        "_COMM=sshd-session",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

    match cmd.spawn() {
        Ok(child) => Ok(child),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(AgentError::PermissionDenied(
                "insufficient permission to read journald SSH logs; \
                 add the service user to the 'systemd-journal' or 'adm' group"
                    .into(),
            ))
        }
        Err(err) => Err(AgentError::Io(err)),
    }
}

#[cfg(target_os = "linux")]
async fn handle_line(agent_id: &str, tx: &mpsc::Sender<SecurityEvent>, line: &str) {
    let Some(parsed) = parse_ssh_log_line(line) else {
        debug!(SSH_LOG_IGNORED, %line);
        return;
    };
    let event = parsed.into_security_event(agent_id, Utc::now());
    if let Err(err) = tx.send(event).await {
        warn_fmt!(SSH_EVENT_ENQUEUE_FAILED, "error" => err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_login_success_publickey() {
        let line = "Accepted publickey for alice from 203.0.113.10 port 52222 ssh2: RSA SHA256:abc";
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshLoginSuccess);
        assert_eq!(event.username.as_deref(), Some("alice"));
        assert_eq!(event.source_ip.as_deref(), Some("203.0.113.10"));
        assert_eq!(event.auth_method.as_deref(), Some("publickey"));
    }

    #[test]
    fn parses_login_failure_password() {
        let line = "Failed password for bob from 198.51.100.20 port 44321 ssh2";
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshLoginFailure);
        assert_eq!(event.username.as_deref(), Some("bob"));
        assert_eq!(event.source_ip.as_deref(), Some("198.51.100.20"));
        assert_eq!(event.auth_method.as_deref(), Some("password"));
    }

    #[test]
    fn classifies_root_login() {
        let line = "Accepted password for root from 192.0.2.5 port 22 ssh2";
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshRootLogin);
        assert_eq!(event.username.as_deref(), Some("root"));
        assert_eq!(event.source_ip.as_deref(), Some("192.0.2.5"));
    }

    #[test]
    fn extracts_invalid_user() {
        let line = "Invalid user admin from 203.0.113.50 port 22";
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshAuthFailure);
        assert_eq!(event.username.as_deref(), Some("admin"));
        assert_eq!(event.source_ip.as_deref(), Some("203.0.113.50"));
    }

    #[test]
    fn parses_failed_invalid_user() {
        let line = "Failed password for invalid user oracle from 10.0.0.8 port 22 ssh2";
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshLoginFailure);
        assert_eq!(event.username.as_deref(), Some("oracle"));
        assert_eq!(event.source_ip.as_deref(), Some("10.0.0.8"));
    }

    #[test]
    fn parses_journal_json_wrapper() {
        let line = r#"{"MESSAGE":"Accepted publickey for alice from 203.0.113.10 port 22 ssh2","SYSLOG_IDENTIFIER":"sshd"}"#;
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshLoginSuccess);
        assert_eq!(event.username.as_deref(), Some("alice"));
    }

    #[test]
    fn parses_sshd_session_syslog_prefix() {
        let line = "Nov 18 01:02:03 host sshd-session[1234]: Accepted publickey for alice from 203.0.113.10 port 22 ssh2";
        let event = parse_ssh_log_line(line).expect("parse");
        assert_eq!(event.event_type, EventType::SshLoginSuccess);
        assert_eq!(event.username.as_deref(), Some("alice"));
    }

    #[test]
    fn ignores_malformed_lines() {
        assert!(parse_ssh_log_line("").is_none());
        assert!(parse_ssh_log_line("not an ssh auth line").is_none());
        assert!(parse_ssh_log_line("Server listening on 0.0.0.0 port 22.").is_none());
        assert!(parse_ssh_log_line("{not-json").is_none());
    }

    #[test]
    fn converts_to_security_event() {
        let parsed =
            parse_ssh_log_line("Accepted publickey for alice from 203.0.113.10 port 22 ssh2")
                .expect("parse");
        let event = parsed.into_security_event("agent-1", Utc::now());
        assert_eq!(event.event_type, EventType::SshLoginSuccess);
        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.username.as_deref(), Some("alice"));
        assert_eq!(event.source_ip.as_deref(), Some("203.0.113.10"));
    }
}
