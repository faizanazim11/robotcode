//! `robotcode testdoc` — wraps `python -m robot.testdoc` via [`tokio::process`].

use std::path::PathBuf;

use anyhow::Result;
use tracing::debug;

/// Arguments for `robotcode testdoc`.
#[derive(Debug, Clone)]
pub struct TestdocArgs {
    /// Path to the Python interpreter (defaults to `python3` on PATH).
    pub python: Option<PathBuf>,
    /// Extra arguments forwarded verbatim to `python -m robot.testdoc`.
    pub args: Vec<String>,
}

/// Generate test documentation with Testdoc.
///
/// Spawns `<python> -m robot.testdoc <args>` and waits for it to finish.
/// Returns the exit code.
pub async fn testdoc(args: TestdocArgs) -> Result<i32> {
    let python = args
        .python
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "python3".to_owned());

    debug!(%python, ?args.args, "Launching testdoc");

    let status = tokio::process::Command::new(&python)
        .args(["-m", "robot.testdoc"])
        .args(&args.args)
        .status()
        .await?;

    Ok(status.code().unwrap_or(1))
}
