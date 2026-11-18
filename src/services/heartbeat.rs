use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::services::api::{ApiClient, HeartbeatPayload};
use crate::services::webhook::WebhookClient;
use crate::{debug, info, warn};

pub struct HeartbeatService {
    api: Arc<ApiClient>,
    webhook: Arc<WebhookClient>,
    agent_id: String,
    interval: Duration,
}

impl HeartbeatService {
    pub fn new(
        api: Arc<ApiClient>,
        webhook: Arc<WebhookClient>,
        agent_id: String,
        interval_seconds: u64,
    ) -> Self {
        Self {
            api,
            webhook,
            agent_id,
            interval: Duration::from_secs(interval_seconds.max(1)),
        }
    }

    pub async fn run(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(HEARTBEAT_STARTED, interval_secs = self.interval.as_secs());
        let mut ticker = tokio::time::interval(self.interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = ticker.tick() => {
                    let payload = HeartbeatPayload {
                        agent_id: self.agent_id.clone(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        status: "running".to_string(),
                        hostname: hostname(),
                        timestamp: Utc::now(),
                    };

                    let mut ok = true;

                    if self.api.is_ready()
                        && let Err(err) = self.api.send_heartbeat(&payload).await
                    {
                        ok = false;
                        warn!(HEARTBEAT_FAILED, error = %err);
                    }

                    if self.webhook.is_enabled() {
                        // Wrap so the receiver can distinguish heartbeat vs security events.
                        let body = serde_json::json!({
                            "type": "AGENT_HEARTBEAT",
                            "payload": payload,
                        });
                        if let Err(err) = self.webhook.post_json(&body).await {
                            ok = false;
                            warn!(WEBHOOK_FAILED, error = %err);
                        }
                    }

                    if ok && (self.api.is_ready() || self.webhook.is_enabled()) {
                        debug!(HEARTBEAT_SENT);
                    }
                }
            }
        }
        info!(HEARTBEAT_STOPPED);
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "unknown".to_string())
}
