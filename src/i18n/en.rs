use super::Msg;

pub fn translate(key: Msg) -> &'static str {
    match key {
        Msg::API_ERROR => {
            "API is not fully configured (base_url/token); \
             events will be processed locally but not published. \
             Set SERVER_GUARD_API_TOKEN and a non-example api.base_url."
        }
        Msg::AGENT_STARTING => "starting server-guard agent",
        Msg::AGENT_STOPPED => "server-guard agent stopped",
        Msg::CONFIG_LOADED => "configuration loaded",
        Msg::RUNTIME_START_FAILED => "failed to start async runtime",
        Msg::AGENT_TERMINATED => "agent terminated with error",
        Msg::SSH_MONITOR_DISABLED => "SSH monitor disabled by configuration",
        Msg::SHUTDOWN_SIGNAL => "shutdown signal received",
        Msg::TASK_ENDED_UNEXPECTEDLY => "background task ended unexpectedly",
        Msg::SSH_MONITOR_STARTED => "SSH monitor started",
        Msg::SSH_MONITOR_STOPPED => "SSH monitor stopped",
        Msg::SSH_JOURNAL_RECONNECT => "SSH journal source ended; reconnecting in {delay}",
        Msg::SSH_PERMISSION_DENIED => "{msg}",
        Msg::SSH_MONITOR_ERROR => "SSH monitor error: {error}; reconnecting in {delay}",
        Msg::SSH_LINUX_ONLY => {
            "SSH journal monitoring is Linux-only; waiting for shutdown on this platform"
        }
        Msg::SSH_EVENT_ENQUEUE_FAILED => "failed to enqueue SSH event: {error}",
        Msg::SSH_LOG_IGNORED => "ignored non-auth SSH log line",
        Msg::HEARTBEAT_STARTED => "heartbeat service started",
        Msg::HEARTBEAT_STOPPED => "heartbeat service stopped",
        Msg::HEARTBEAT_SENT => "heartbeat sent",
        Msg::HEARTBEAT_FAILED => "heartbeat failed; will retry",
        Msg::API_SKIP_EVENT => "API not configured; skipping event publish",
        Msg::API_EVENT_PUBLISHED => "event published",
        Msg::API_SKIP_HEARTBEAT => "API not configured; skipping heartbeat",
        Msg::API_FLUSH_FAILED => "failed to flush event on shutdown",
        Msg::API_PUBLISH_FAILED => "failed to publish event",
        Msg::WEBHOOK_ENABLED => "webhook delivery enabled",
        Msg::WEBHOOK_DISABLED => "webhook_api_url is empty; webhook delivery disabled",
        Msg::WEBHOOK_SENT => "webhook delivered",
        Msg::WEBHOOK_FAILED => "webhook delivery failed; will continue monitoring",
        Msg::WEBHOOK_SKIP => "webhook not configured; skipping delivery",
        Msg::DELIVERY_NOT_CONFIGURED => {
            "no delivery target configured (webhook_api_url / api); \
             monitoring continues locally only"
        }
        Msg::RECEIVED_SIGTERM => "received SIGTERM",
        Msg::RECEIVED_SIGINT => "received SIGINT",
        Msg::RECEIVED_CTRL_C => "received Ctrl-C",
    }
}
