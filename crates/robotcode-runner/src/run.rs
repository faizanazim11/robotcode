//! `robotcode run` — wraps `python -m robot` via [`tokio::process`].

use std::path::PathBuf;

use anyhow::Result;
use tracing::debug;

/// Arguments for `robotcode run`.
#[derive(Debug, Clone)]
pub struct RunArgs {
    /// Path to the Python interpreter (defaults to `python3` on PATH).
    pub python: Option<PathBuf>,
    /// Extra arguments forwarded verbatim to `python -m robot`.
    pub args: Vec<String>,
}

/// Run Robot Framework tests.
///
/// Spawns `<python> -m robot <args>` and waits for it to finish,
/// streaming stdout/stderr to the current process. Returns the exit code.
pub async fn run(args: RunArgs) -> Result<i32> {
    let python = args
        .python
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "python3".to_owned());

    debug!(%python, ?args.args, "Launching robot runner");

    let status = tokio::process::Command::new(&python)
        .args(["-m", "robot"])
        .args(&args.args)
        .status()
        .await?;

    Ok(status.code().unwrap_or(1))
}
