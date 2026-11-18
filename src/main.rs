use std::path::PathBuf;
use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

use server_guard::agent::AgentRunner;
use server_guard::config::Config;
use server_guard::i18n;
use server_guard::{error_msg, info};

fn main() -> ExitCode {
    // Fail fast on incomplete first-boot config when running under systemd,
    // so RestartSec does not thrash when only the example placeholder is present.
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("SERVER_GUARD_CONFIG").map(PathBuf::from));

    let settings = i18n::load_settings(None).unwrap_or_default();
    let language = i18n::load_language(None);
    i18n::init(language);

    let config = match Config::load(config_path.as_deref()) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("server-guard: failed to load configuration: {err}");
            // Exit code 78 (EX_CONFIG) signals a configuration problem to systemd.
            // Combined with packaging defaults this avoids infinite restart loops.
            return ExitCode::from(78);
        }
    };

    init_tracing(&config.agent.log_level);

    info!(
        CONFIG_LOADED,
        agent_id = %config.agent.id,
        language = language.as_str(),
        webhook = settings.webhook_enabled()
    );

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("server-guard")
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            error_msg!(RUNTIME_START_FAILED, error = %err);
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(AgentRunner::new(config, settings).run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error_msg!(AGENT_TERMINATED, error = %err);
            ExitCode::FAILURE
        }
    }
}

fn init_tracing(log_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stdout)
        .init();
}
