use crate::info;

/// Wait until SIGINT or SIGTERM is received (Unix). On non-Unix, wait for Ctrl-C only.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");
        let mut sigint =
            signal(SignalKind::interrupt()).expect("failed to register SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => info!(RECEIVED_SIGTERM),
            _ = sigint.recv() => info!(RECEIVED_SIGINT),
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        info!(RECEIVED_CTRL_C);
    }
}
