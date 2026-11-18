use std::time::Duration;

use reqwest::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Serialize;

use crate::error::{AgentError, Result};
use crate::i18n::Settings;
use crate::{debug, warn};

const ENV_WEBHOOK_URL: &str = "SERVER_GUARD_WEBHOOK_API_URL";

/// Optional webhook delivery to the operator's central server.
/// Disabled when `webhook_api_url` is empty / unset.
#[derive(Clone)]
pub struct WebhookClient {
    client: Client,
    url: Option<String>,
}

impl WebhookClient {
    pub fn new(settings: &Settings, timeout_seconds: u64) -> Result<Self> {
        let url = resolve_webhook_url(settings);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds.max(1)))
            .default_headers(headers)
            .user_agent(format!("server-guard/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|err| AgentError::Api(err.to_string()))?;

        Ok(Self { client, url })
    }

    pub fn is_enabled(&self) -> bool {
        self.url.is_some()
    }

    /// POST arbitrary JSON payload to the webhook URL. No-op when disabled.
    pub async fn post_json<T: Serialize + ?Sized>(&self, payload: &T) -> Result<()> {
        let Some(url) = self.url.as_deref() else {
            debug!(WEBHOOK_SKIP);
            return Ok(());
        };

        match self.client.post(url).json(payload).send().await {
            Ok(response) if response.status().is_success() => {
                debug!(WEBHOOK_SENT);
                Ok(())
            }
            Ok(response) => {
                let status = response.status();
                Err(AgentError::Api(format!(
                    "webhook returned unexpected status {status}"
                )))
            }
            Err(err) => {
                // Never log the full URL query/fragment if secrets were embedded.
                warn!(WEBHOOK_FAILED, error = %sanitize_error(&err));
                Err(AgentError::Api(sanitize_error(&err)))
            }
        }
    }
}

fn resolve_webhook_url(settings: &Settings) -> Option<String> {
    if let Ok(value) = std::env::var(ENV_WEBHOOK_URL) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.trim_end_matches('/').to_string());
        }
    }

    let trimmed = settings.webhook_api_url.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.trim_end_matches('/').to_string())
    }
}

fn sanitize_error(err: &reqwest::Error) -> String {
    let mut msg = err.to_string();
    if let Some(start) = msg.find("Bearer ") {
        let end = msg[start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .map(|i| start + i)
            .unwrap_or(msg.len());
        msg.replace_range(start..end, "Bearer [REDACTED]");
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{LocaleSettings, Settings};

    #[test]
    fn disabled_when_url_empty() {
        let settings = Settings {
            locale: LocaleSettings::default(),
            webhook_api_url: String::new(),
        };
        let client = WebhookClient::new(&settings, 10).expect("client");
        assert!(!client.is_enabled());
    }

    #[test]
    fn enabled_when_url_set() {
        let settings = Settings {
            locale: LocaleSettings::default(),
            webhook_api_url: "https://hooks.example.com/ingest".into(),
        };
        let client = WebhookClient::new(&settings, 10).expect("client");
        assert!(client.is_enabled());
    }
}
