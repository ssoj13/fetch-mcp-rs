use std::path::PathBuf;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize logging based on transport mode
/// - stdio mode: File logging only (NO stderr to avoid MCP handshake issues)
/// - stream mode: Console + optional file logging
pub fn init_logging(log_file: Option<PathBuf>, is_stream_mode: bool) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(env_filter);

    if is_stream_mode {
        // HTTP stream mode: console + optional file
        let console_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        if let Some(path) = log_file {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("Failed to open log file");

            let file_layer = fmt::layer()
                .with_writer(std::sync::Arc::new(file))
                .with_ansi(false);

            registry
                .with(console_layer)
                .with(file_layer)
                .init();
        } else {
            registry.with(console_layer).init();
        }
    } else {
        // stdio mode: ONLY file logging (no stderr)
        if let Some(path) = log_file {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("Failed to open log file");

            let file_layer = fmt::layer()
                .with_writer(std::sync::Arc::new(file))
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true);

            registry.with(file_layer).init();
        } else {
            // No logging in stdio mode without -l flag
            registry.init();
        }
    }
}
