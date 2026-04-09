//! Keyword evaluation engine.
//!
//! Evaluates a Robot Framework keyword call by forwarding it to the Python
//! bridge's `evaluate` method.  The bridge spawns a minimal RF execution
//! context and runs the keyword in-process, returning the return value and
//! any log messages produced.
//!
//! # Note on the bridge `evaluate` method
//!
//! The Python helper exposes an `evaluate` method that accepts:
//!
//! ```json
//! {
//!   "keyword": "Log  hello  level=INFO",
//!   "variables": { "${VAR}": "value" },
//!   "python_path": ["/path/to/site-packages"]
//! }
//! ```
//!
//! and returns:
//!
//! ```json
//! { "result": null, "log": ["INFO: hello"], "error": null }
//! ```

use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request sent to the Python bridge for a single keyword evaluation.
#[derive(Debug, Serialize)]
pub struct EvalRequest {
    /// The keyword call expression (e.g. `"Log  hello  level=INFO"`).
    pub keyword: String,
    /// Variable bindings active in this REPL session.
    pub variables: HashMap<String, String>,
    /// Extra Python paths to add before invoking RF.
    pub python_path: Vec<String>,
}

/// Response returned by the Python bridge after evaluation.
#[derive(Debug, Serialize, Deserialize)]
pub struct EvalResponse {
    /// Return value of the keyword (JSON-serialised), if any.
    pub result: Option<Value>,
    /// Log messages emitted by the keyword.
    pub log: Vec<String>,
    /// Error message if the keyword raised an exception.
    pub error: Option<String>,
}

/// Evaluate a single keyword call using the provided bridge.
///
/// Returns an [`EvalResponse`] on success, or a bridge/transport error.
pub async fn evaluate<B: robotcode_python_bridge::Bridge>(
    bridge: &B,
    keyword: impl Into<String>,
    variables: HashMap<String, String>,
    python_path: Vec<String>,
) -> Result<EvalResponse> {
    let req = EvalRequest {
        keyword: keyword.into(),
        variables,
        python_path,
    };
    let params = serde_json::to_value(&req)?;
    let raw = bridge.call("evaluate", params).await?;
    let resp: EvalResponse = serde_json::from_value(raw)?;
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_python_bridge::MockBridge;
    use serde_json::json;
    use std::future::Future;
    use std::pin::Pin;

    /// A test bridge that echoes back a canned evaluate response.
    struct EchoBridge;

    impl robotcode_python_bridge::Bridge for EchoBridge {
        fn call<'a>(
            &'a self,
            _method: &'a str,
            params: serde_json::Value,
        ) -> Pin<
            Box<
                dyn Future<Output = robotcode_python_bridge::Result<serde_json::Value>> + Send + 'a,
            >,
        > {
            let keyword = params
                .get("keyword")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            Box::pin(async move {
                Ok(json!({
                    "result": null,
                    "log": [format!("INFO: evaluated {keyword}")],
                    "error": null
                }))
            })
        }
    }

    #[tokio::test]
    async fn evaluate_returns_log() {
        let bridge = EchoBridge;
        let resp = evaluate(&bridge, "Log  hello", HashMap::new(), vec![])
            .await
            .expect("evaluate failed");
        assert!(resp.error.is_none());
        assert_eq!(resp.log, vec!["INFO: evaluated Log  hello"]);
    }

    #[tokio::test]
    async fn mock_bridge_roundtrip() {
        // MockBridge returns a fixed JSON value for any call.
        let bridge = MockBridge::with_responses([(
            "evaluate",
            vec![json!({
                "result": "42",
                "log": [],
                "error": null
            })],
        )]);
        let resp = evaluate(&bridge, "Get Length  ${LIST}", HashMap::new(), vec![])
            .await
            .expect("evaluate failed");
        assert_eq!(resp.result, Some(json!("42")));
    }
}
