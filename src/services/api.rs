use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};

use crate::config::Config;
use crate::error::{AgentError, Result};
use crate::events::SecurityEvent;
use crate::services::webhook::WebhookClient;
use crate::{debug, warn};

/// HTTPS client for the central API. Designed so a local queue / retry layer
/// can wrap `send_event` later without changing call sites.
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    ready: bool,
}

impl ApiClient {
    pub fn new(config: &Config) -> Result<Self> {
        let timeout = Duration::from_secs(config.api.timeout_seconds);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(token) = config.api.token.as_deref() {
            // Never log the token value.
            let value = format!("Bearer {token}");
            let header = HeaderValue::from_str(&value)
                .map_err(|err| AgentError::Config(format!("invalid API token header: {err}")))?;
            headers.insert(AUTHORIZATION, header);
        }

        let client = Client::builder()
            .timeout(timeout)
            .default_headers(headers)
            .user_agent(format!("server-guard/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|err| AgentError::Api(err.to_string()))?;

        Ok(Self {
            client,
            base_url: config.api.base_url.trim_end_matches('/').to_string(),
            ready: config.is_api_ready(),
        })
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub async fn send_event(&self, event: &SecurityEvent) -> Result<()> {
        if !self.ready {
            debug!(API_SKIP_EVENT, event_type = event.event_type.as_str());
            return Ok(());
        }

        let url = format!("{}/v1/events", self.base_url);
        match self.client.post(&url).json(event).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!(
                        API_EVENT_PUBLISHED,
                        event_id = %event.event_id,
                        event_type = event.event_type.as_str()
                    );
                    Ok(())
                } else {
                    let status = response.status();
                    // Avoid logging response bodies that might echo secrets.
                    Err(AgentError::Api(format!(
                        "unexpected status {status} publishing event {}",
                        event.event_id
                    )))
                }
            }
            Err(err) => Err(AgentError::Api(sanitize_reqwest_error(&err))),
        }
    }

    pub async fn send_heartbeat(&self, payload: &HeartbeatPayload) -> Result<()> {
        if !self.ready {
            debug!(API_SKIP_HEARTBEAT);
            return Ok(());
        }

        let url = format!("{}/v1/heartbeat", self.base_url);
        match self.client.post(&url).json(payload).send().await {
            Ok(response) if response.status().is_success() => Ok(()),
            Ok(response) => Err(AgentError::Api(format!(
                "unexpected status {} for heartbeat",
                response.status()
            ))),
            Err(err) => Err(AgentError::Api(sanitize_reqwest_error(&err))),
        }
    }
}

/// Strip potential credential material from reqwest error display strings.
fn sanitize_reqwest_error(err: &reqwest::Error) -> String {
    let mut msg = err.to_string();
    // Best-effort redaction if a URL with userinfo ever appears.
    if let Some(start) = msg.find("Bearer ") {
        let end = msg[start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .map(|i| start + i)
            .unwrap_or(msg.len());
        msg.replace_range(start..end, "Bearer [REDACTED]");
    }
    msg
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HeartbeatPayload {
    pub agent_id: String,
    pub version: String,
    pub status: String,
    pub hostname: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Deliver a security event to API and/or webhook. Failures are logged; agent keeps running.
async fn deliver_event(api: &ApiClient, webhook: &WebhookClient, event: &SecurityEvent) {
    if api.is_ready()
        && let Err(err) = api.send_event(event).await
    {
        warn!(
            API_PUBLISH_FAILED,
            error = %err,
            event_type = event.event_type.as_str()
        );
    }

    if webhook.is_enabled()
        && let Err(err) = webhook.post_json(event).await
    {
        warn!(
            WEBHOOK_FAILED,
            error = %err,
            event_type = event.event_type.as_str()
        );
    }
}

/// Background worker that drains the event channel and posts to API / webhook.
pub async fn event_publisher(
    api: Arc<ApiClient>,
    webhook: Arc<WebhookClient>,
    mut rx: tokio::sync::mpsc::Receiver<SecurityEvent>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    while let Ok(event) = rx.try_recv() {
                        if api.is_ready()
                            && let Err(err) = api.send_event(&event).await
                        {
                            warn!(API_FLUSH_FAILED, error = %err);
                        }
                        if webhook.is_enabled()
                            && let Err(err) = webhook.post_json(&event).await
                        {
                            warn!(WEBHOOK_FAILED, error = %err);
                        }
                    }
                    break;
                }
            }
            maybe_event = rx.recv() => {
                match maybe_event {
                    Some(event) => deliver_event(&api, &webhook, &event).await,
                    None => break,
                }
            }
        }
    }
}
