//! `robotcode rebot` — wraps `python -m robot.rebot` via [`tokio::process`].

use std::path::PathBuf;

use anyhow::Result;
use tracing::debug;

/// Arguments for `robotcode rebot`.
#[derive(Debug, Clone)]
pub struct RebotArgs {
    /// Path to the Python interpreter (defaults to `python3` on PATH).
    pub python: Option<PathBuf>,
    /// Extra arguments forwarded verbatim to `python -m robot.rebot`.
    pub args: Vec<String>,
}

/// Post-process Robot Framework output files with Rebot.
///
/// Spawns `<python> -m robot.rebot <args>` and waits for it to finish.
/// Returns the exit code.
pub async fn rebot(args: RebotArgs) -> Result<i32> {
    let python = args
        .python
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "python3".to_owned());

    debug!(%python, ?args.args, "Launching rebot");

    let status = tokio::process::Command::new(&python)
        .args(["-m", "robot.rebot"])
        .args(&args.args)
        .status()
        .await?;

    Ok(status.code().unwrap_or(1))
}
