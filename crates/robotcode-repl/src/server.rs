//! JSON-RPC 2.0 REPL server (stdio or TCP).
//!
//! The REPL server accepts newline-delimited JSON-RPC 2.0 requests on its
//! input stream and writes responses to its output stream.
//!
//! ## Supported methods
//!
//! | Method | Params | Result |
//! |--------|--------|--------|
//! | `evaluate` | `{ "keyword": str, "variables": obj, "python_path": [str] }` | `EvalResponse` |
//! | `history` | `{}` | `[HistoryEntry]` |
//! | `history/clear` | `{}` | `{}` |
//! | `complete` | `{ "prefix": str }` | `[str]` |
//! | `shutdown` | `{}` | `{}` |

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

use crate::eval;
use crate::history::History;

// ── JSON-RPC 2.0 types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl Response {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ── ReplServer ────────────────────────────────────────────────────────────────

/// Configuration for the REPL server.
#[derive(Debug, Default)]
pub struct ReplConfig {
    /// Path to the Python interpreter used for the bridge.
    pub python: Option<PathBuf>,
}

/// The REPL server state shared across request handlers.
struct ServerState {
    history: History,
    python: Option<PathBuf>,
}

impl ServerState {
    fn new(config: ReplConfig) -> Self {
        Self {
            history: History::new(),
            python: config.python,
        }
    }

    /// Handle a single JSON-RPC request and return the response value.
    ///
    /// Errors carry the JSON-RPC error code alongside the message:
    /// - `-32601` Method not found
    /// - `-32602` Invalid params
    /// - `-32603` Internal error
    async fn handle(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> std::result::Result<Value, (i32, String)> {
        match method {
            "evaluate" => self.handle_evaluate(params).await,
            "history" => Ok(json!(self.history.entries())),
            "history/clear" => {
                self.history.clear();
                Ok(json!({}))
            }
            "complete" => self.handle_complete(params),
            "shutdown" => Ok(json!({})),
            unknown => Err((-32601, format!("Method not found: {unknown}"))),
        }
    }

    async fn handle_evaluate(
        &self,
        params: Option<Value>,
    ) -> std::result::Result<Value, (i32, String)> {
        let params = params.unwrap_or_default();

        let keyword = params
            .get("keyword")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        if keyword.is_empty() {
            return Err((-32602, "'keyword' parameter is required".into()));
        }

        let variables: HashMap<String, String> = params
            .get("variables")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let python_path: Vec<String> = params
            .get("python_path")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        debug!(keyword = %keyword, "evaluate");

        // Build a subprocess bridge pointing to the configured Python interpreter.
        // Resolve the helper.py script from the standard bundled layout.
        // Expected layouts:
        //   Production (installed extension):
        //     <extension-root>/bundled/bin/robotcode        (the binary itself)
        //     <extension-root>/bundled/libs/python_bridge/helper.py
        //   The binary sits at <extension-root>/bundled/bin/, so two parents up
        //   from current_exe() gives <extension-root>/.
        //   Development (cargo run from repo root):
        //     Falls back to "python-bridge/helper.py" relative to CWD.
        let python_exe = self
            .python
            .clone()
            .unwrap_or_else(|| PathBuf::from("python3"));

        let helper_path = std::env::current_exe()
            .ok()
            .and_then(|exe| {
                // <extension-root>/bundled/bin/robotcode
                //   -> exe.parent()  = <extension-root>/bundled/bin
                //   -> .parent()     = <extension-root>/bundled
                //   -> .join("libs/python_bridge/helper.py")
                exe.parent()
                    .and_then(|bin_dir| bin_dir.parent())
                    .map(|bundled| bundled.join("libs").join("python_bridge").join("helper.py"))
            })
            .filter(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("python-bridge/helper.py"));

        let bridge = robotcode_python_bridge::SubprocessBridge::new(&python_exe, &helper_path);

        let resp = eval::evaluate(&bridge, &keyword, variables, python_path).await;

        match resp {
            Ok(eval_resp) => {
                let error_flag = eval_resp.error.is_some();
                let result_str = eval_resp.result.as_ref().map(|v| v.to_string());
                self.history.push(keyword, result_str, error_flag);
                Ok(serde_json::to_value(&eval_resp).expect("serialise eval response"))
            }
            Err(e) => {
                self.history.push(keyword, None, true);
                Err((-32603, e.to_string()))
            }
        }
    }

    fn handle_complete(&self, params: Option<Value>) -> std::result::Result<Value, (i32, String)> {
        let prefix = params
            .as_ref()
            .and_then(|v| v.get("prefix"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        // Return history entries whose expression starts with the prefix.
        let matches: Vec<String> = self
            .history
            .entries()
            .into_iter()
            .filter(|e| e.expression.to_lowercase().starts_with(&prefix))
            .map(|e| e.expression)
            .collect();

        Ok(json!(matches))
    }
}

// ── serve_stdio ───────────────────────────────────────────────────────────────

/// Run the REPL server on stdin/stdout.
pub async fn serve_stdio(config: ReplConfig) -> Result<()> {
    info!("Starting REPL server (stdio)");
    let state = Arc::new(ServerState::new(config));

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    serve_streams(state, stdin, stdout).await
}

// ── serve_tcp ─────────────────────────────────────────────────────────────────

/// Accept a single TCP connection and run the REPL server on it.
pub async fn serve_tcp(port: u16, config: ReplConfig) -> Result<()> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    info!(%addr, "REPL server listening on TCP");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let (stream, peer) = listener.accept().await?;
    info!(%peer, "REPL server accepted connection");

    let state = Arc::new(ServerState::new(config));
    let (read, write) = tokio::io::split(stream);
    serve_streams(state, read, write).await
}

// ── internal ─────────────────────────────────────────────────────────────────

async fn serve_streams<R, W>(state: Arc<ServerState>, reader: R, mut writer: W) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_owned();
        if line.is_empty() {
            continue;
        }

        debug!(line = %line, "received line");

        // Per JSON-RPC 2.0: a request without an `id` is a *notification* —
        // the server executes it but MUST NOT send a response.
        let response: Option<Response> = match serde_json::from_str::<Request>(&line) {
            Err(e) => {
                warn!(error = %e, "invalid JSON-RPC request");
                Some(Response::error(None, -32700, format!("Parse error: {e}")))
            }
            Ok(req) => {
                let is_notification = req.id.is_none();
                let id = req.id.clone();
                match state.handle(&req.method, req.params).await {
                    Ok(result) => {
                        if req.method == "shutdown" {
                            if !is_notification {
                                let resp = Response::success(id, result);
                                let mut line = serde_json::to_string(&resp)?;
                                line.push('\n');
                                writer.write_all(line.as_bytes()).await?;
                                writer.flush().await?;
                            }
                            info!("REPL server shutdown");
                            return Ok(());
                        }
                        if is_notification {
                            None
                        } else {
                            Some(Response::success(id, result))
                        }
                    }
                    Err((code, msg)) => {
                        error!(method = %req.method, code, error = %msg, "handler error");
                        if is_notification {
                            None
                        } else {
                            Some(Response::error(id, code, msg))
                        }
                    }
                }
            }
        };

        if let Some(resp) = response {
            let mut out = serde_json::to_string(&resp)?;
            out.push('\n');
            writer.write_all(out.as_bytes()).await?;
            writer.flush().await?;
        }
    }

    info!("REPL server input stream closed");
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    async fn roundtrip(input_lines: &[&str]) -> Vec<Value> {
        let state = Arc::new(ServerState::new(ReplConfig::default()));
        let (client_stream, server_stream) = duplex(4096);
        let (server_read, server_write) = tokio::io::split(server_stream);

        let state_clone = state.clone();
        let server = tokio::spawn(async move {
            serve_streams(state_clone, server_read, server_write)
                .await
                .ok();
        });

        let (client_read, mut client_write) = tokio::io::split(client_stream);
        for line in input_lines {
            client_write.write_all(line.as_bytes()).await.unwrap();
            client_write.write_all(b"\n").await.unwrap();
        }
        drop(client_write); // EOF

        server.await.unwrap();

        let mut reader = BufReader::new(client_read).lines();
        let mut results = Vec::new();
        while let Some(line) = reader.next_line().await.unwrap() {
            results.push(serde_json::from_str::<Value>(&line).unwrap());
        }
        results
    }

    #[tokio::test]
    async fn history_empty_on_start() {
        let responses = roundtrip(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"history","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}"#,
        ])
        .await;
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["result"], json!([]));
        assert_eq!(responses[1]["result"], json!({}));
    }

    #[tokio::test]
    async fn history_clear() {
        let state = Arc::new(ServerState::new(ReplConfig::default()));
        state.history.push("Log  hi".into(), None, false);

        let result = state.handle("history/clear", None).await.unwrap();
        assert_eq!(result, json!({}));
        assert!(state.history.entries().is_empty());
    }

    #[tokio::test]
    async fn complete_by_prefix() {
        let state = Arc::new(ServerState::new(ReplConfig::default()));
        state.history.push("Log  hello".into(), None, false);
        state.history.push("Log  world".into(), None, false);
        state.history.push("Get Length  ${x}".into(), None, false);

        let result = state
            .handle("complete", Some(json!({"prefix": "log"})))
            .await
            .unwrap();
        let matches: Vec<String> = serde_json::from_value(result).unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|m| m.starts_with("Log")));
    }

    #[tokio::test]
    async fn method_not_found_returns_32601() {
        let state = Arc::new(ServerState::new(ReplConfig::default()));
        let err = state.handle("no_such_method", None).await.unwrap_err();
        assert_eq!(err.0, -32601);
    }

    #[tokio::test]
    async fn invalid_params_returns_32602() {
        let state = Arc::new(ServerState::new(ReplConfig::default()));
        // evaluate with empty keyword should return -32602 Invalid params.
        let err = state
            .handle("evaluate", Some(json!({"keyword": ""})))
            .await
            .unwrap_err();
        assert_eq!(err.0, -32602);
    }

    #[tokio::test]
    async fn notification_produces_no_response() {
        // A request without "id" is a notification; no response should be sent.
        let responses = roundtrip(&[
            // notification (no id)
            r#"{"jsonrpc":"2.0","method":"history/clear","params":{}}"#,
            // regular request to verify server is still alive
            r#"{"jsonrpc":"2.0","id":1,"method":"history","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}"#,
        ])
        .await;
        // Only the two requests with an id should produce responses.
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["id"], json!(1));
        assert_eq!(responses[1]["id"], json!(2));
    }

    #[tokio::test]
    async fn parse_error_returns_rpc_error() {
        let responses = roundtrip(&[
            "not json",
            r#"{"jsonrpc":"2.0","id":1,"method":"shutdown","params":{}}"#,
        ])
        .await;
        assert!(responses[0].get("error").is_some());
        assert_eq!(responses[0]["error"]["code"], json!(-32700));
    }
}
