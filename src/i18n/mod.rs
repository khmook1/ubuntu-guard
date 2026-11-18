//! Compile-time language packs and runtime locale selection.
//!
//! Usage:
//! ```ignore
//! use crate::i18n::Msg;
//! warn!(API_ERROR); // via crate::i18n::{warn, info, error, debug}
//! ```

mod en;
mod ko;

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Message keys shared by every language pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum Msg {
    /// API base_url/token incomplete — events stay local.
    API_ERROR,
    AGENT_STARTING,
    AGENT_STOPPED,
    CONFIG_LOADED,
    RUNTIME_START_FAILED,
    AGENT_TERMINATED,
    SSH_MONITOR_DISABLED,
    SHUTDOWN_SIGNAL,
    TASK_ENDED_UNEXPECTEDLY,
    SSH_MONITOR_STARTED,
    SSH_MONITOR_STOPPED,
    SSH_JOURNAL_RECONNECT,
    SSH_PERMISSION_DENIED,
    SSH_MONITOR_ERROR,
    SSH_LINUX_ONLY,
    SSH_EVENT_ENQUEUE_FAILED,
    SSH_LOG_IGNORED,
    HEARTBEAT_STARTED,
    HEARTBEAT_STOPPED,
    HEARTBEAT_SENT,
    HEARTBEAT_FAILED,
    API_SKIP_EVENT,
    API_EVENT_PUBLISHED,
    API_SKIP_HEARTBEAT,
    API_FLUSH_FAILED,
    API_PUBLISH_FAILED,
    WEBHOOK_ENABLED,
    WEBHOOK_DISABLED,
    WEBHOOK_SENT,
    WEBHOOK_FAILED,
    WEBHOOK_SKIP,
    DELIVERY_NOT_CONFIGURED,
    RECEIVED_SIGTERM,
    RECEIVED_SIGINT,
    RECEIVED_CTRL_C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ko,
}

impl Language {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "ko" | "kr" | "korean" | "ko-kr" => Self::Ko,
            _ => Self::En,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ko => "ko",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Settings {
    #[serde(default)]
    pub locale: LocaleSettings,
    /// Central server webhook URL. Empty disables webhook delivery.
    #[serde(default)]
    pub webhook_api_url: String,
}

impl Settings {
    pub fn webhook_enabled(&self) -> bool {
        !self.webhook_api_url.trim().is_empty()
            || std::env::var("SERVER_GUARD_WEBHOOK_API_URL")
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocaleSettings {
    #[serde(default = "default_language_string")]
    pub language: String,
}

impl Default for LocaleSettings {
    fn default() -> Self {
        Self {
            language: default_language_string(),
        }
    }
}

fn default_language_string() -> String {
    "en".to_string()
}

const DEFAULT_SETTINGS_PATHS: &[&str] = &[
    "/etc/server-guard/settings.toml",
    "config/settings.toml",
    "config/settings.example.toml",
];

const ENV_LANGUAGE: &str = "SERVER_GUARD_LANGUAGE";
const ENV_SETTINGS_PATH: &str = "SERVER_GUARD_SETTINGS";

static LANGUAGE: OnceLock<Language> = OnceLock::new();

/// Initialize the process-wide language from settings / env.
pub fn init(language: Language) {
    let _ = LANGUAGE.set(language);
}

pub fn current_language() -> Language {
    *LANGUAGE.get().unwrap_or(&Language::En)
}

/// Resolve language: env → settings file → default (`en`).
pub fn load_language(settings_path: Option<&Path>) -> Language {
    if let Ok(value) = std::env::var(ENV_LANGUAGE)
        && !value.trim().is_empty()
    {
        return Language::parse(&value);
    }

    if let Some(settings) = load_settings(settings_path) {
        return Language::parse(&settings.locale.language);
    }

    Language::En
}

pub fn load_settings(explicit: Option<&Path>) -> Option<Settings> {
    let path = resolve_settings_path(explicit)?;
    let raw = fs::read_to_string(path).ok()?;
    toml::from_str(&raw).ok()
}

fn resolve_settings_path(explicit: Option<&Path>) -> Option<std::path::PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    if let Ok(path) = std::env::var(ENV_SETTINGS_PATH) {
        return Some(std::path::PathBuf::from(path));
    }
    for candidate in DEFAULT_SETTINGS_PATHS {
        let path = std::path::PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Look up a message in the active language pack (fallback: English).
pub fn t(key: Msg) -> &'static str {
    match current_language() {
        Language::Ko => ko::translate(key),
        Language::En => en::translate(key),
    }
}

/// Replace `{name}` placeholders in a translated template.
pub fn t_fmt(key: Msg, pairs: &[(&str, &str)]) -> String {
    let mut out = t(key).to_string();
    for (name, value) in pairs {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// `warn!(API_ERROR)` — looks up `Msg::API_ERROR` in the active language pack.
#[macro_export]
macro_rules! warn {
    ($key:ident) => {{
        ::tracing::warn!("{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
    ($key:ident, $($field:tt)*) => {{
        ::tracing::warn!($($field)*, "{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
}

/// `info!(AGENT_STARTING)` — language-pack backed info log.
#[macro_export]
macro_rules! info {
    ($key:ident) => {{
        ::tracing::info!("{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
    ($key:ident, $($field:tt)*) => {{
        ::tracing::info!($($field)*, "{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
}

/// `error_msg!(AGENT_TERMINATED)` — language-pack backed error log.
/// Named `error_msg` to avoid clashing with the `error` module.
#[macro_export]
macro_rules! error_msg {
    ($key:ident) => {{
        ::tracing::error!("{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
    ($key:ident, $($field:tt)*) => {{
        ::tracing::error!($($field)*, "{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
}

/// `debug!(HEARTBEAT_SENT)` — language-pack backed debug log.
#[macro_export]
macro_rules! debug {
    ($key:ident) => {{
        ::tracing::debug!("{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
    ($key:ident, $($field:tt)*) => {{
        ::tracing::debug!($($field)*, "{}", $crate::i18n::t($crate::i18n::Msg::$key));
    }};
}

/// Log a key whose template contains `{placeholders}` filled at runtime.
#[macro_export]
macro_rules! warn_fmt {
    ($key:ident, $($name:literal => $value:expr),+ $(,)?) => {{
        let __msg = $crate::i18n::t_fmt(
            $crate::i18n::Msg::$key,
            &[$(($name, &$value.to_string())),+],
        );
        ::tracing::warn!("{__msg}");
    }};
}

#[macro_export]
macro_rules! error_fmt {
    ($key:ident, $($name:literal => $value:expr),+ $(,)?) => {{
        let __msg = $crate::i18n::t_fmt(
            $crate::i18n::Msg::$key,
            &[$(($name, &$value.to_string())),+],
        );
        ::tracing::error!("{__msg}");
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_korean_api_error() {
        let msg = ko::translate(Msg::API_ERROR);
        assert!(msg.contains("API"));
        assert!(msg.contains("설정"));
    }

    #[test]
    fn translates_english_api_error() {
        assert_eq!(Language::parse("ko"), Language::Ko);
        assert_eq!(Language::parse("en"), Language::En);
        let msg = en::translate(Msg::API_ERROR);
        assert!(msg.contains("API"));
        assert!(msg.contains("configured"));
    }

    #[test]
    fn parses_settings_example() {
        let raw = include_str!("../../config/settings.example.toml");
        let settings: Settings = toml::from_str(raw).expect("settings parse");
        assert_eq!(Language::parse(&settings.locale.language), Language::En);
        assert!(settings.webhook_api_url.is_empty());
        assert!(!settings.webhook_enabled());
    }

    #[test]
    fn fmt_replaces_placeholders() {
        let filled = {
            // Force English path without relying on process-wide OnceLock.
            let mut out = en::translate(Msg::SSH_JOURNAL_RECONNECT).to_string();
            out = out.replace("{delay}", "3s");
            out
        };
        assert!(filled.contains("3s"));
        assert!(!filled.contains("{delay}"));
    }

    #[test]
    fn every_key_exists_in_both_packs() {
        let keys = [
            Msg::API_ERROR,
            Msg::AGENT_STARTING,
            Msg::AGENT_STOPPED,
            Msg::CONFIG_LOADED,
            Msg::RUNTIME_START_FAILED,
            Msg::AGENT_TERMINATED,
            Msg::SSH_MONITOR_DISABLED,
            Msg::SHUTDOWN_SIGNAL,
            Msg::TASK_ENDED_UNEXPECTEDLY,
            Msg::SSH_MONITOR_STARTED,
            Msg::SSH_MONITOR_STOPPED,
            Msg::SSH_JOURNAL_RECONNECT,
            Msg::SSH_PERMISSION_DENIED,
            Msg::SSH_MONITOR_ERROR,
            Msg::SSH_LINUX_ONLY,
            Msg::SSH_EVENT_ENQUEUE_FAILED,
            Msg::SSH_LOG_IGNORED,
            Msg::HEARTBEAT_STARTED,
            Msg::HEARTBEAT_STOPPED,
            Msg::HEARTBEAT_SENT,
            Msg::HEARTBEAT_FAILED,
            Msg::API_SKIP_EVENT,
            Msg::API_EVENT_PUBLISHED,
            Msg::API_SKIP_HEARTBEAT,
            Msg::API_FLUSH_FAILED,
            Msg::API_PUBLISH_FAILED,
            Msg::WEBHOOK_ENABLED,
            Msg::WEBHOOK_DISABLED,
            Msg::WEBHOOK_SENT,
            Msg::WEBHOOK_FAILED,
            Msg::WEBHOOK_SKIP,
            Msg::DELIVERY_NOT_CONFIGURED,
            Msg::RECEIVED_SIGTERM,
            Msg::RECEIVED_SIGINT,
            Msg::RECEIVED_CTRL_C,
        ];
        for key in keys {
            assert!(!en::translate(key).is_empty());
            assert!(!ko::translate(key).is_empty());
        }
    }
}
