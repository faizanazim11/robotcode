//! `robotcode-python-bridge` — async bridge to the Robot Framework Python runtime.
//!
//! This crate provides the [`Bridge`] trait and two implementations:
//!
//! * [`SubprocessBridge`] — spawns `python helper.py` and communicates via
//!   newline-delimited JSON over stdio.  This is the default production bridge.
//! * [`MockBridge`] — returns hard-coded responses; used in unit tests that must
//!   not require a Python interpreter.

pub mod error;
pub mod mock;
pub mod subprocess;
pub mod types;

pub use error::BridgeError;
pub use mock::MockBridge;
pub use subprocess::SubprocessBridge;
pub use types::*;

use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

/// Result alias used throughout this crate.
pub type Result<T> = std::result::Result<T, BridgeError>;

/// Core trait for all bridge implementations.
///
/// Every method maps 1-to-1 to the `helper.py` method dispatch table.
pub trait Bridge: Send + Sync {
    /// Call an arbitrary method on the Python bridge.
    fn call<'a>(
        &'a self,
        method: &'a str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;

    /// Return the Robot Framework version from the Python runtime.
    fn rf_version(&self) -> Pin<Box<dyn Future<Output = Result<RfVersion>> + Send + '_>> {
        let fut = self.call("rf_version", Value::Object(Default::default()));
        Box::pin(async move {
            let val = fut.await?;
            Ok(serde_json::from_value(val)?)
        })
    }

    /// Normalize a string using RF's NormalizedDict rules.
    fn normalize(
        &self,
        value: &str,
        remove_underscores: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        let params = serde_json::json!({
            "value": value,
            "remove_underscores": remove_underscores,
        });
        let fut = self.call("normalize", params);
        Box::pin(async move {
            let val = fut.await?;
            let obj = val
                .as_object()
                .and_then(|o| o.get("normalized"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            Ok(obj)
        })
    }

    /// Introspect a Robot Framework keyword library.
    fn library_doc(
        &self,
        params: LibraryDocParams,
    ) -> Pin<Box<dyn Future<Output = Result<LibraryDoc>> + Send + '_>> {
        let json_params = serde_json::to_value(params).unwrap_or_default();
        let fut = self.call("library_doc", json_params);
        Box::pin(async move {
            let val = fut.await?;
            Ok(serde_json::from_value(val)?)
        })
    }

    /// Load a Robot Framework variables file.
    fn variables_doc(
        &self,
        params: VariablesDocParams,
    ) -> Pin<Box<dyn Future<Output = Result<VariablesDoc>> + Send + '_>> {
        let json_params = serde_json::to_value(params).unwrap_or_default();
        let fut = self.call("variables_doc", json_params);
        Box::pin(async move {
            let val = fut.await?;
            Ok(serde_json::from_value(val)?)
        })
    }

    /// Parse embedded argument patterns from a keyword name.
    fn embedded_args(
        &self,
        pattern: &str,
    ) -> Pin<Box<dyn Future<Output = Result<EmbeddedArgs>> + Send + '_>> {
        let params = serde_json::json!({ "pattern": pattern });
        let fut = self.call("embedded_args", params);
        Box::pin(async move {
            let val = fut.await?;
            Ok(serde_json::from_value(val)?)
        })
    }

    /// Discover tests in the given paths using RF's TestSuiteBuilder.
    fn discover(
        &self,
        params: DiscoverParams,
    ) -> Pin<Box<dyn Future<Output = Result<DiscoverResult>> + Send + '_>> {
        let json_params = serde_json::to_value(params).unwrap_or_default();
        let fut = self.call("discover", json_params);
        Box::pin(async move {
            let val = fut.await?;
            Ok(serde_json::from_value(val)?)
        })
    }
}
