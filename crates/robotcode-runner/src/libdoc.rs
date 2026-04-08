//! `robotcode libdoc` — wraps `python -m robot.libdoc` via [`tokio::process`].

use std::path::PathBuf;

use anyhow::Result;
use tracing::debug;

/// Arguments for `robotcode libdoc`.
#[derive(Debug, Clone)]
pub struct LibdocArgs {
    /// Path to the Python interpreter (defaults to `python3` on PATH).
    pub python: Option<PathBuf>,
    /// Extra arguments forwarded verbatim to `python -m robot.libdoc`.
    pub args: Vec<String>,
}

/// Generate library documentation with Libdoc.
///
/// Spawns `<python> -m robot.libdoc <args>` and waits for it to finish.
/// Returns the exit code.
pub async fn libdoc(args: LibdocArgs) -> Result<i32> {
    let python = args
        .python
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "python3".to_owned());

    debug!(%python, ?args.args, "Launching libdoc");

    let status = tokio::process::Command::new(&python)
        .args(["-m", "robot.libdoc"])
        .args(&args.args)
        .status()
        .await?;

    Ok(status.code().unwrap_or(1))
}
