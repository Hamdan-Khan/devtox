pub fn init_tracing() {
    #[cfg(feature = "dev-tracing")]
    {
        use std::fs::OpenOptions;
        use tracing_subscriber::{EnvFilter, fmt};

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("dev.log")
            .expect("failed to open log file");

        fmt()
            .with_writer(std::sync::Mutex::new(file))
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
            )
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .init();

        tracing::info!("tracing initialized -> dev.log");
    }
}
