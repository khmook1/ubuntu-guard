use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Security and lifecycle events emitted by the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    SshLoginSuccess,
    SshLoginFailure,
    SshRootLogin,
    SshAuthFailure,
    AgentStarted,
    AgentStopped,
    AgentHeartbeat,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SshLoginSuccess => "SSH_LOGIN_SUCCESS",
            Self::SshLoginFailure => "SSH_LOGIN_FAILURE",
            Self::SshRootLogin => "SSH_ROOT_LOGIN",
            Self::SshAuthFailure => "SSH_AUTH_FAILURE",
            Self::AgentStarted => "AGENT_STARTED",
            Self::AgentStopped => "AGENT_STOPPED",
            Self::AgentHeartbeat => "AGENT_HEARTBEAT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecurityEvent {
    pub event_id: Uuid,
    pub agent_id: String,
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl SecurityEvent {
    pub fn new(agent_id: impl Into<String>, event_type: EventType, severity: Severity) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            agent_id: agent_id.into(),
            event_type,
            timestamp: Utc::now(),
            source_ip: None,
            username: None,
            severity,
            metadata: None,
        }
    }

    pub fn with_source_ip(mut self, ip: impl Into<String>) -> Self {
        self.source_ip = Some(ip.into());
        self
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_event_json() {
        let event = SecurityEvent::new("agent-1", EventType::SshLoginSuccess, Severity::Info)
            .with_source_ip("203.0.113.10")
            .with_username("alice")
            .with_metadata(json!({"auth_method": "publickey"}));

        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["event_type"], "SSH_LOGIN_SUCCESS");
        assert_eq!(value["agent_id"], "agent-1");
        assert_eq!(value["source_ip"], "203.0.113.10");
        assert_eq!(value["username"], "alice");
        assert_eq!(value["severity"], "info");
        assert_eq!(value["metadata"]["auth_method"], "publickey");
        assert!(value.get("event_id").is_some());
        assert!(value.get("timestamp").is_some());
    }

    #[test]
    fn omits_optional_none_fields() {
        let event = SecurityEvent::new("agent-1", EventType::AgentHeartbeat, Severity::Info);
        let value = serde_json::to_value(&event).expect("serialize");
        assert!(value.get("source_ip").is_none());
        assert!(value.get("username").is_none());
        assert!(value.get("metadata").is_none());
    }
}
