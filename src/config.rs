use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AgentError, Result};

const DEFAULT_CONFIG_PATHS: &[&str] = &[
    "/etc/server-guard/config.toml",
    "config/config.toml",
    "config/config.example.toml",
];

const ENV_CONFIG_PATH: &str = "SERVER_GUARD_CONFIG";
const ENV_API_TOKEN: &str = "SERVER_GUARD_API_TOKEN";
const ENV_AGENT_ID: &str = "SERVER_GUARD_AGENT_ID";
const ENV_API_BASE_URL: &str = "SERVER_GUARD_API_BASE_URL";
const ENV_LOG_LEVEL: &str = "SERVER_GUARD_LOG_LEVEL";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub agent: AgentConfig,
    pub api: ApiConfig,
    pub monitor: MonitorConfig,
    pub heartbeat: HeartbeatConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiConfig {
    pub base_url: String,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    /// Injected from environment; never loaded from the example config.
    #[serde(default, skip_serializing)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorConfig {
    #[serde(default = "default_true")]
    pub ssh_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_heartbeat_interval")]
    pub interval_seconds: u64,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_timeout_seconds() -> u64 {
    10
}

fn default_heartbeat_interval() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agent: AgentConfig {
                id: String::new(),
                log_level: default_log_level(),
            },
            api: ApiConfig {
                base_url: "https://api.example.com".to_string(),
                timeout_seconds: default_timeout_seconds(),
                token: None,
            },
            monitor: MonitorConfig { ssh_enabled: true },
            heartbeat: HeartbeatConfig {
                enabled: true,
                interval_seconds: default_heartbeat_interval(),
            },
        }
    }
}

impl Config {
    /// Load configuration from an explicit path or the standard search paths.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = resolve_config_path(path)?;
        let raw = fs::read_to_string(&path).map_err(|err| {
            AgentError::Config(format!("failed to read {}: {err}", path.display()))
        })?;
        let mut config = Self::parse_toml(&raw)?;
        config.apply_env_overrides();
        config.ensure_agent_id();
        config.validate()?;
        Ok(config)
    }

    pub fn parse_toml(raw: &str) -> Result<Self> {
        toml::from_str(raw).map_err(|err| AgentError::Config(format!("invalid TOML: {err}")))
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(id) = env::var(ENV_AGENT_ID)
            && !id.trim().is_empty()
        {
            self.agent.id = id;
        }
        if let Ok(level) = env::var(ENV_LOG_LEVEL)
            && !level.trim().is_empty()
        {
            self.agent.log_level = level;
        }
        if let Ok(url) = env::var(ENV_API_BASE_URL)
            && !url.trim().is_empty()
        {
            self.api.base_url = url;
        }
        if let Ok(token) = env::var(ENV_API_TOKEN)
            && !token.trim().is_empty()
        {
            self.api.token = Some(token);
        }
    }

    fn ensure_agent_id(&mut self) {
        if self.agent.id.trim().is_empty() {
            self.agent.id = uuid::Uuid::new_v4().to_string();
        }
    }

    /// Returns true when the agent has enough configuration to talk to the API.
    pub fn is_api_ready(&self) -> bool {
        !self.api.base_url.trim().is_empty()
            && self.api.base_url != "https://api.example.com"
            && self
                .api
                .token
                .as_ref()
                .is_some_and(|t| !t.trim().is_empty())
    }

    pub fn validate(&self) -> Result<()> {
        if self.api.base_url.trim().is_empty() {
            return Err(AgentError::Config("api.base_url must not be empty".into()));
        }
        if self.api.timeout_seconds == 0 {
            return Err(AgentError::Config(
                "api.timeout_seconds must be greater than 0".into(),
            ));
        }
        if self.heartbeat.enabled && self.heartbeat.interval_seconds == 0 {
            return Err(AgentError::Config(
                "heartbeat.interval_seconds must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

fn resolve_config_path(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Ok(path) = env::var(ENV_CONFIG_PATH) {
        return Ok(PathBuf::from(path));
    }
    for candidate in DEFAULT_CONFIG_PATHS {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(AgentError::Config(format!(
        "no configuration file found; set {ENV_CONFIG_PATH} or create one of: {}",
        DEFAULT_CONFIG_PATHS.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_config() {
        let raw = include_str!("../config/config.example.toml");
        let config = Config::parse_toml(raw).expect("example config should parse");
        assert_eq!(config.agent.log_level, "info");
        assert_eq!(config.api.base_url, "https://api.example.com");
        assert_eq!(config.api.timeout_seconds, 10);
        assert!(config.monitor.ssh_enabled);
        assert!(config.heartbeat.enabled);
        assert_eq!(config.heartbeat.interval_seconds, 60);
        assert!(config.api.token.is_none());
    }

    #[test]
    fn rejects_zero_timeout() {
        let mut config = Config::default();
        config.api.timeout_seconds = 0;
        assert!(config.validate().is_err());
    }
}
