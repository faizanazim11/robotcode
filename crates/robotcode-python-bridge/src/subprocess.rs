//! [`SubprocessBridge`] — spawns `python helper.py` and communicates via
//! newline-delimited JSON (NDJSON) over stdio.
//!
//! # Lifecycle
//!
//! 1. Call [`SubprocessBridge::start`] (or simply make any request — the
//!    bridge auto-starts on first use via [`Bridge::call`]).
//! 2. Each call to [`Bridge::call`] sends one JSON line to stdin and awaits
//!    the matching response line from stdout.
//! 3. If the subprocess exits unexpectedly between requests, the next call
//!    to [`Bridge::call`] will detect the dead process (via `try_wait`),
//!    apply a 1-second back-off, and restart it automatically.
//! 4. Call [`SubprocessBridge::stop`] to tear down the current subprocess.
//!    This closes stdin (causing `helper.py` to exit) but does not wait for
//!    the process to finish.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::{Bridge, BridgeError, Result};

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

/// Per-request map: ID → sender half of a oneshot channel.
type PendingMap = DashMap<u64, oneshot::Sender<Result<Value>>>;

/// Inner bridge state that multiple concurrent callers share via an `Arc`.
struct BridgeInner {
    /// Exclusive access to stdin for sending requests.
    stdin: Mutex<ChildStdin>,
    /// In-flight requests waiting for a response.
    pending: Arc<PendingMap>,
    /// The child process, behind a mutex only for `try_wait` liveness checks.
    child: std::sync::Mutex<Child>,
}

impl BridgeInner {
    /// Returns `true` if the child process is still running.
    fn is_alive(&self) -> bool {
        self.child
            .lock()
            .map(|mut c| c.try_wait().map(|s| s.is_none()).unwrap_or(false))
            .unwrap_or(false)
    }
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
    /// Current bridge inner state, if any.
    ///
    /// A `RwLock` is used so that concurrent `call()` invocations can all
    /// hold a *read* lock simultaneously while only lifecycle operations
    /// (`start`/`stop`/`ensure_running`) need the write lock.
    state: RwLock<Option<Arc<BridgeInner>>>,
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
            state: RwLock::new(None),
        }
    }

    /// Start (or restart) the Python subprocess.
    pub async fn start(&self) -> Result<()> {
        let mut guard = self.state.write().await;
        *guard = Some(Arc::new(self.spawn_child().await?));
        Ok(())
    }

    /// Stop the Python subprocess by dropping the inner state.
    ///
    /// Closing the `Arc<BridgeInner>` closes stdin (causing `helper.py` to
    /// exit its read loop).  The process is also killed on drop because it
    /// was spawned with `kill_on_drop(true)`.
    pub async fn stop(&self) {
        let mut guard = self.state.write().await;
        *guard = None;
    }

    /// Ensure the bridge is running.
    ///
    /// * If no subprocess has ever been started, starts one.
    /// * If the subprocess has exited unexpectedly, waits 1 second and
    ///   restarts it (back-off is applied only for unexpected exits).
    async fn ensure_running(&self) -> Result<()> {
        // Fast path: read lock — check if process is alive.
        let needs_restart;
        let is_crash;
        {
            let guard = self.state.read().await;
            match guard.as_ref() {
                None => {
                    needs_restart = true;
                    is_crash = false;
                }
                Some(inner) => {
                    let alive = inner.is_alive();
                    needs_restart = !alive;
                    is_crash = !alive;
                }
            }
        }

        if !needs_restart {
            return Ok(());
        }

        if is_crash {
            warn!("Python bridge subprocess exited unexpectedly; restarting after 1s back-off");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // Write lock — double-check and (re)start.
        let mut guard = self.state.write().await;
        let still_needs = match guard.as_ref() {
            None => true,
            Some(inner) => !inner.is_alive(),
        };
        if still_needs {
            *guard = Some(Arc::new(self.spawn_child().await?));
        }
        Ok(())
    }

    async fn spawn_child(&self) -> Result<BridgeInner> {
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

        let pending: Arc<PendingMap> = Arc::new(DashMap::new());
        let pending_clone = Arc::clone(&pending);

        // Spawn a reader task that distributes responses to waiters.
        tokio::spawn(Self::reader_task(stdout, pending_clone));

        Ok(BridgeInner {
            stdin: Mutex::new(stdin),
            pending,
            child: std::sync::Mutex::new(child),
        })
    }

    async fn reader_task(stdout: ChildStdout, pending: Arc<PendingMap>) {
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

                            if let Some((_, tx)) = pending.remove(&id) {
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
                    let keys: Vec<u64> = pending.iter().map(|e| *e.key()).collect();
                    for k in keys {
                        if let Some((_, tx)) = pending.remove(&k) {
                            let _ = tx.send(Err(BridgeError::ProcessExited));
                        }
                    }
                    break;
                }
                Err(e) => {
                    error!(error = %e, "Bridge stdout read error");
                    // Drain pending so callers don't hang until the 30s timeout.
                    let keys: Vec<u64> = pending.iter().map(|e| *e.key()).collect();
                    for k in keys {
                        if let Some((_, tx)) = pending.remove(&k) {
                            let _ = tx.send(Err(BridgeError::ProcessExited));
                        }
                    }
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

            // Clone Arc<BridgeInner> while holding a brief read lock, then
            // release the lock so other concurrent calls are not blocked.
            let inner = {
                let guard = self.state.read().await;
                Arc::clone(guard.as_ref().ok_or(BridgeError::NotRunning)?)
            };

            // Register the oneshot channel BEFORE writing, so we can never
            // miss a response that arrives while we're still in write_all.
            let (tx, rx) = oneshot::channel::<Result<Value>>();
            inner.pending.insert(id, tx);

            // Write the request to stdin (only stdin is locked here).
            {
                let mut stdin = inner.stdin.lock().await;
                if let Err(e) = stdin.write_all(line.as_bytes()).await {
                    inner.pending.remove(&id);
                    return Err(e.into());
                }
                if let Err(e) = stdin.flush().await {
                    inner.pending.remove(&id);
                    return Err(e.into());
                }
            }

            // Wait for the response with a timeout.
            match tokio::time::timeout(Duration::from_secs(30), rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(BridgeError::ProcessExited),
                Err(_) => {
                    inner.pending.remove(&id);
                    Err(BridgeError::Timeout)
                }
            }
        })
    }
}
