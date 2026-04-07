//! `tracing` subscriber setup.
//!
//! Port of the Python `robotcode.core.utils.logging` module.

use tracing_subscriber::{fmt, EnvFilter};

/// Initialise the global `tracing` subscriber.
///
/// Respects the `RUST_LOG` environment variable for log-level filtering.
/// Calling this more than once is a no-op (the second call silently fails).
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Initialise logging with an explicit filter string (e.g. `"debug,hyper=warn"`).
pub fn init_logging_with_filter(filter: &str) {
    let _ = fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::new(filter))
        .with_writer(std::io::stderr)
        .try_init();
}
