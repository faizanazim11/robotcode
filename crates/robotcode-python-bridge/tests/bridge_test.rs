//! Integration tests for `SubprocessBridge` — require Python + robotframework.
//!
//! These tests are gated behind the `CI` environment variable so they only run
//! in CI and in local development when Python is configured.

use std::path::PathBuf;

use robotcode_python_bridge::{Bridge, MockBridge, SubprocessBridge};
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Locate `python3` on PATH; skip if not found.
fn find_python() -> Option<PathBuf> {
    for name in &["python3", "python"] {
        if python_exists(name) {
            return Some(PathBuf::from(name));
        }
    }
    None
}

fn python_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Path to `python-bridge/helper.py` relative to the workspace root.
fn helper_path() -> PathBuf {
    // This file is at crates/robotcode-python-bridge/tests/bridge_test.rs
    // → go up 4 levels to reach workspace root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../python-bridge/helper.py")
}

// ---------------------------------------------------------------------------
// MockBridge tests (no Python required)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mock_bridge_rf_version() {
    let bridge = MockBridge::with_responses([(
        "rf_version",
        vec![json!({"version":"7.0.0","major":7,"minor":0,"patch":0})],
    )]);

    let v = bridge.rf_version().await.unwrap();
    assert_eq!(v.version, "7.0.0");
    assert_eq!(v.major, 7);
    assert_eq!(v.minor, 0);
    assert_eq!(v.patch, 0);
}

#[tokio::test]
async fn mock_bridge_normalize() {
    let bridge =
        MockBridge::with_responses([("normalize", vec![json!({"normalized":"mykeywordname"})])]);

    let n = bridge.normalize("My Keyword Name", true).await.unwrap();
    assert_eq!(n, "mykeywordname");
}

#[tokio::test]
async fn mock_bridge_library_doc() {
    let bridge = MockBridge::with_responses([(
        "library_doc",
        vec![json!({
            "name": "BuiltIn",
            "doc": "Robot Framework built-in library.",
            "version": "7.0.0",
            "scope": "GLOBAL",
            "named_args": true,
            "keywords": [
                {
                    "name": "Log",
                    "args": [
                        {"name":"message","kind":"POSITIONAL_OR_NAMED","default":null,"types":["str"]}
                    ],
                    "doc": "Logs the given message.",
                    "tags": [],
                    "source": null,
                    "lineno": null
                }
            ],
            "inits": [],
            "typedocs": []
        })],
    )]);

    use robotcode_python_bridge::LibraryDocParams;
    let doc = bridge
        .library_doc(LibraryDocParams {
            name: "BuiltIn".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(doc.name, "BuiltIn");
    assert_eq!(doc.keywords.len(), 1);
    assert_eq!(doc.keywords[0].name, "Log");
}

#[tokio::test]
async fn mock_bridge_embedded_args() {
    let bridge = MockBridge::with_responses([(
        "embedded_args",
        vec![json!({
            "name": "the user ${name} logs in",
            "args": ["name"],
            "regex": "^the user (.+) logs in$"
        })],
    )]);

    let ea = bridge
        .embedded_args("the user ${name} logs in")
        .await
        .unwrap();
    assert_eq!(ea.args, vec!["name"]);
    assert!(!ea.regex.is_empty());
}

#[tokio::test]
async fn mock_bridge_queue_exhausted_returns_error() {
    let bridge = MockBridge::empty();
    let result = bridge.rf_version().await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// SubprocessBridge integration tests (require Python + robotframework)
// ---------------------------------------------------------------------------

/// Macro that skips the test if Python is not available.
macro_rules! require_python {
    ($python:ident, $helper:ident) => {
        let python_path = match find_python() {
            Some(p) => p,
            None => {
                eprintln!("Skipping test: python not found on PATH");
                return;
            }
        };
        let $helper = helper_path();
        if !$helper.exists() {
            eprintln!(
                "Skipping test: helper.py not found at {}",
                $helper.display()
            );
            return;
        }
        let $python = python_path;
    };
}

#[tokio::test]
async fn subprocess_bridge_rf_version() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    let v = bridge.rf_version().await.unwrap();
    assert!(v.major >= 4, "Expected RF >= 4, got {}", v.version);

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_normalize() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    let n = bridge.normalize("My Keyword Name", true).await.unwrap();
    assert_eq!(n, "mykeywordname");

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_library_doc_builtin() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    use robotcode_python_bridge::LibraryDocParams;
    let doc = bridge
        .library_doc(LibraryDocParams {
            name: "BuiltIn".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(doc.name, "BuiltIn");
    // BuiltIn should have at least 50 keywords across all RF versions.
    assert!(
        doc.keywords.len() >= 50,
        "Expected >= 50 BuiltIn keywords, got {}",
        doc.keywords.len()
    );

    // Verify "Log" keyword exists.
    let log_kw = doc.keywords.iter().find(|k| k.name == "Log");
    assert!(log_kw.is_some(), "BuiltIn should have a 'Log' keyword");

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_library_doc_collections() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    use robotcode_python_bridge::LibraryDocParams;
    let doc = bridge
        .library_doc(LibraryDocParams {
            name: "Collections".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(doc.name, "Collections");
    assert!(
        doc.keywords.len() >= 20,
        "Expected >= 20 Collections keywords, got {}",
        doc.keywords.len()
    );

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_library_doc_string() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    use robotcode_python_bridge::LibraryDocParams;
    let doc = bridge
        .library_doc(LibraryDocParams {
            name: "String".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(doc.name, "String");
    assert!(
        doc.keywords.len() >= 15,
        "Expected >= 15 String keywords, got {}",
        doc.keywords.len()
    );

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_library_doc_operating_system() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    use robotcode_python_bridge::LibraryDocParams;
    let doc = bridge
        .library_doc(LibraryDocParams {
            name: "OperatingSystem".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(doc.name, "OperatingSystem");
    assert!(
        doc.keywords.len() >= 30,
        "Expected >= 30 OperatingSystem keywords, got {}",
        doc.keywords.len()
    );

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_embedded_args() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    let ea = bridge
        .embedded_args("the user ${name} logs in with password ${password}")
        .await
        .unwrap();

    assert!(
        ea.args.contains(&"name".to_owned()),
        "Expected 'name' in embedded args: {:?}",
        ea.args
    );
    assert!(
        ea.args.contains(&"password".to_owned()),
        "Expected 'password' in embedded args: {:?}",
        ea.args
    );

    bridge.stop().await;
}

#[tokio::test]
async fn subprocess_bridge_multiple_requests_single_process() {
    require_python!(python, helper);
    let bridge = SubprocessBridge::new(&python, &helper);
    bridge.start().await.unwrap();

    // Fire 5 requests to the same bridge instance to verify the request-ID
    // tracking works correctly.
    for _ in 0..5 {
        let v = bridge.rf_version().await.unwrap();
        assert!(v.major >= 4);
    }

    bridge.stop().await;
}
