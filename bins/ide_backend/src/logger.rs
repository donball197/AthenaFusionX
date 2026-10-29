//! Logger — initialises tracing subscriber with env-filter and JSON output.
use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("nexus_ide_backend=info,nexus_runtime_core=info,warn"));

    fmt().with_env_filter(filter).with_target(true).with_thread_ids(false).compact().init();

    tracing::info!(">> [LOGGER] Tracing substrate online");
}
