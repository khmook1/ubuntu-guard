use std::sync::Arc;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::config::Config;
use crate::events::{EventType, SecurityEvent, Severity};
use crate::i18n::Settings;
use crate::monitors::SshMonitor;
use crate::services::{ApiClient, HeartbeatService, WebhookClient, event_publisher};
use crate::utils::shutdown_signal;
use crate::{error_msg, info, warn};

const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// Orchestrates monitors, heartbeat, and graceful shutdown.
pub struct AgentRunner {
    config: Config,
    settings: Settings,
}

impl AgentRunner {
    pub fn new(config: Config, settings: Settings) -> Self {
        Self { config, settings }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let agent_id = self.config.agent.id.clone();
        info!(
            AGENT_STARTING,
            %agent_id,
            version = env!("CARGO_PKG_VERSION")
        );

        let api = Arc::new(ApiClient::new(&self.config)?);
        let webhook = Arc::new(WebhookClient::new(
            &self.settings,
            self.config.api.timeout_seconds,
        )?);

        if webhook.is_enabled() {
            info!(WEBHOOK_ENABLED);
        } else {
            info!(WEBHOOK_DISABLED);
        }

        if !api.is_ready() && !webhook.is_enabled() {
            warn!(DELIVERY_NOT_CONFIGURED);
        }

        let (event_tx, event_rx) = mpsc::channel::<SecurityEvent>(EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let mut tasks = JoinSet::new();

        // Event publisher (API + webhook)
        {
            let api = Arc::clone(&api);
            let webhook = Arc::clone(&webhook);
            let rx_shutdown = shutdown_rx.clone();
            tasks.spawn(async move {
                event_publisher(api, webhook, event_rx, rx_shutdown).await;
            });
        }

        // Heartbeat
        if self.config.heartbeat.enabled {
            let heartbeat = HeartbeatService::new(
                Arc::clone(&api),
                Arc::clone(&webhook),
                agent_id.clone(),
                self.config.heartbeat.interval_seconds,
            );
            let rx_shutdown = shutdown_rx.clone();
            tasks.spawn(async move {
                heartbeat.run(rx_shutdown).await;
            });
        }

        // SSH monitor
        if self.config.monitor.ssh_enabled {
            let monitor = SshMonitor::new(agent_id.clone(), event_tx.clone());
            let rx_shutdown = shutdown_rx.clone();
            tasks.spawn(async move {
                monitor.run(rx_shutdown).await;
            });
        } else {
            info!(SSH_MONITOR_DISABLED);
        }

        // Lifecycle: started
        let _ = event_tx
            .send(
                SecurityEvent::new(agent_id.clone(), EventType::AgentStarted, Severity::Info)
                    .with_metadata(serde_json::json!({
                        "version": env!("CARGO_PKG_VERSION"),
                    })),
            )
            .await;

        // Wait for SIGTERM / SIGINT
        shutdown_signal().await;
        info!(SHUTDOWN_SIGNAL);
        let _ = shutdown_tx.send(true);

        // Lifecycle: stopped (best-effort before channel closes)
        let _ = event_tx.try_send(SecurityEvent::new(
            agent_id,
            EventType::AgentStopped,
            Severity::Info,
        ));
        drop(event_tx);

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(()) => {}
                Err(err) if err.is_cancelled() => {}
                Err(err) => error_msg!(TASK_ENDED_UNEXPECTEDLY, error = %err),
            }
        }

        info!(AGENT_STOPPED);
        Ok(())
    }
}
