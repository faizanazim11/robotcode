//! [`MockBridge`] — an in-memory bridge implementation for unit tests.
//!
//! Call [`MockBridge::with_responses`] to register canned responses keyed by
//! method name.  Each registered response is returned exactly once in FIFO
//! order; the bridge returns an error once the queue for that method is
//! exhausted.

use std::collections::HashMap;
use std::pin::Pin;

use serde_json::Value;
use tokio::sync::Mutex;

use crate::{Bridge, BridgeError, Result};

/// A test double for the Python bridge.
///
/// # Example
/// ```rust,no_run
/// use robotcode_python_bridge::{Bridge, MockBridge};
/// use serde_json::json;
///
/// # async fn run() {
/// let bridge = MockBridge::with_responses([(
///     "rf_version",
///     vec![json!({"version":"7.0.0","major":7,"minor":0,"patch":0})],
/// )]);
///
/// let version = bridge.rf_version().await.unwrap();
/// assert_eq!(version.major, 7);
/// # }
/// ```
pub struct MockBridge {
    responses: Mutex<HashMap<String, std::collections::VecDeque<Value>>>,
}

impl MockBridge {
    /// Create a `MockBridge` with no pre-loaded responses (all calls fail).
    pub fn empty() -> Self {
        Self {
            responses: Mutex::new(HashMap::new()),
        }
    }

    /// Create a `MockBridge` with pre-loaded responses.
    ///
    /// `responses` is an iterable of `(method_name, [response1, response2, …])`.
    pub fn with_responses<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = (S, Vec<Value>)>,
        S: Into<String>,
    {
        let map = responses
            .into_iter()
            .map(|(k, v)| (k.into(), v.into_iter().collect()))
            .collect();
        Self {
            responses: Mutex::new(map),
        }
    }
}

impl Bridge for MockBridge {
    fn call<'a>(
        &'a self,
        method: &'a str,
        _params: Value,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>> {
        let method = method.to_owned();
        Box::pin(async move {
            let mut map = self.responses.lock().await;
            let queue = map.get_mut(&method).ok_or_else(|| {
                BridgeError::Internal(format!("MockBridge: no response registered for {method:?}"))
            })?;
            queue.pop_front().ok_or_else(|| {
                BridgeError::Internal(format!(
                    "MockBridge: response queue exhausted for {method:?}"
                ))
            })
        })
    }
}
