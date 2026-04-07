//! [`SubprocessBridge`] — spawns `python helper.py` and communicates via
//! newline-delimited JSON (NDJSON) over stdio.
//!
//! # Lifecycle
//!
//! 1. Call [`SubprocessBridge::start`] before the first request.
//! 2. Each call to [`Bridge::call`] sends one JSON line to stdin and awaits
//!    the matching response line from stdout.
//! 3. If the subprocess crashes, [`SubprocessBridge::start`] restarts it with
//!    a 1-second back-off.
//! 4. Call [`SubprocessBridge::stop`] to send a graceful shutdown (closes
//!    stdin) and wait for the process to exit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info, warn};

use crate::{Bridge, BridgeError, Result};

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

type Pending = HashMap<u64, oneshot::Sender<Result<Value>>>;

struct BridgeState {
    stdin: ChildStdin,
    pending: Arc<Mutex<Pending>>,
    _child: Child,
}

// ---------------------------------------------------------------------------
// SubprocessBridge
// ---------------------------------------------------------------------------

/// Communicates with a long-lived `python helper.py` subprocess.
pub struct SubprocessBridge {
    /// Path to the Python interpreter.
    python: PathBuf,
    /// Absolute path to `helper.py`.
    helper: PathBuf,
    /// Monotonically increasing request ID counter.
    next_id: AtomicU64,
    /// Mutable inner state, protected by a mutex so the bridge can be
    /// shared across async tasks.
    state: Mutex<Option<BridgeState>>,
}

impl SubprocessBridge {
    /// Create a new bridge (does **not** start the subprocess yet).
    ///
    /// * `python` — path to the Python interpreter (e.g. `python3` or
    ///   `/usr/bin/python3`).
    /// * `helper` — path to `python-bridge/helper.py`.
    pub fn new(python: impl AsRef<Path>, helper: impl AsRef<Path>) -> Self {
        Self {
            python: python.as_ref().to_owned(),
            helper: helper.as_ref().to_owned(),
            next_id: AtomicU64::new(1),
            state: Mutex::new(None),
        }
    }

    /// Start (or restart) the Python subprocess.
    pub async fn start(&self) -> Result<()> {
        let mut guard = self.state.lock().await;
        *guard = Some(self.spawn_child().await?);
        Ok(())
    }

    /// Stop the Python subprocess gracefully by closing stdin.
    pub async fn stop(&self) {
        let mut guard = self.state.lock().await;
        *guard = None; // drops Child, closing stdin/stdout
    }

    /// Ensure the bridge is running, starting it if necessary.
    async fn ensure_running(&self) -> Result<()> {
        let guard = self.state.lock().await;
        if guard.is_none() {
            drop(guard);
            self.start().await?;
        }
        Ok(())
    }

    async fn spawn_child(&self) -> Result<BridgeState> {
        use tokio::process::Command;

        info!(
            python = %self.python.display(),
            helper = %self.helper.display(),
            "Spawning Python bridge subprocess"
        );

        let mut child = Command::new(&self.python)
            .arg(&self.helper)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| BridgeError::Internal("failed to open child stdin".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BridgeError::Internal("failed to open child stdout".to_owned()))?;

        let pending: Arc<Mutex<Pending>> = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        // Spawn a reader task that distributes responses to waiters.
        tokio::spawn(Self::reader_task(stdout, pending_clone));

        Ok(BridgeState {
            stdin,
            pending,
            _child: child,
        })
    }

    async fn reader_task(stdout: ChildStdout, pending: Arc<Mutex<Pending>>) {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    debug!(line = %line, "Bridge stdout line");
                    match serde_json::from_str::<Value>(&line) {
                        Ok(val) => {
                            let id = val.get("id").and_then(|v| v.as_u64()).unwrap_or(u64::MAX);

                            let result: Result<Value> = if let Some(err) = val.get("error") {
                                let code =
                                    err.get("code").and_then(|c| c.as_i64()).unwrap_or(-32000);
                                let message = err
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("unknown error")
                                    .to_owned();
                                Err(BridgeError::Python { code, message })
                            } else if let Some(result) = val.get("result") {
                                Ok(result.clone())
                            } else {
                                Err(BridgeError::Internal(
                                    "response has neither result nor error".to_owned(),
                                ))
                            };

                            let mut map = pending.lock().await;
                            if let Some(tx) = map.remove(&id) {
                                let _ = tx.send(result);
                            } else {
                                warn!(id, "No pending waiter for bridge response");
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Invalid JSON from bridge stdout");
                        }
                    }
                }
                Ok(None) => {
                    info!("Bridge stdout closed (process exited)");
                    // Fail all pending waiters.
                    let mut map = pending.lock().await;
                    for (_, tx) in map.drain() {
                        let _ = tx.send(Err(BridgeError::ProcessExited));
                    }
                    break;
                }
                Err(e) => {
                    error!(error = %e, "Bridge stdout read error");
                    break;
                }
            }
        }
    }
}

impl Bridge for SubprocessBridge {
    fn call<'a>(
        &'a self,
        method: &'a str,
        params: Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            // Ensure the bridge process is running.
            self.ensure_running().await?;

            let id = self.next_id.fetch_add(1, Ordering::SeqCst);
            let request = serde_json::json!({
                "id": id,
                "method": method,
                "params": params,
            });
            let mut line = serde_json::to_string(&request)?;
            line.push('\n');

            // Register a oneshot channel before writing the request, to avoid
            // a race where the response arrives before we register.
            let (tx, rx) = oneshot::channel::<Result<Value>>();
            {
                let mut guard = self.state.lock().await;
                let state = guard.as_mut().ok_or(BridgeError::NotRunning)?;
                state.pending.lock().await.insert(id, tx);
                state.stdin.write_all(line.as_bytes()).await?;
                state.stdin.flush().await?;
            }

            // Wait for the response with a timeout.
            match tokio::time::timeout(Duration::from_secs(30), rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(BridgeError::ProcessExited),
                Err(_) => {
                    // Clean up the pending entry.
                    let guard = self.state.lock().await;
                    if let Some(state) = guard.as_ref() {
                        state.pending.lock().await.remove(&id);
                    }
                    Err(BridgeError::Timeout)
                }
            }
        })
    }
}
