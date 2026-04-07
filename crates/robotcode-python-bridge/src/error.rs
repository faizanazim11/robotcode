//! Error types for the Python bridge.

use thiserror::Error;

/// Errors that can occur when communicating with the Python bridge.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// The Python subprocess has not been started yet.
    #[error("bridge is not running")]
    NotRunning,

    /// The Python subprocess exited unexpectedly.
    #[error("bridge process exited unexpectedly")]
    ProcessExited,

    /// I/O error communicating with the subprocess (stdin/stdout).
    #[error("bridge I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The response from Python was not valid JSON.
    #[error("bridge JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The Python bridge returned an explicit error response.
    #[error("bridge returned error (code {code}): {message}")]
    Python { code: i64, message: String },

    /// A timeout waiting for the Python response.
    #[error("bridge request timed out")]
    Timeout,

    /// The in-flight request map is poisoned (internal error).
    #[error("bridge internal error: {0}")]
    Internal(String),
}
