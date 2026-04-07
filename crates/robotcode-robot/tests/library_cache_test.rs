//! Tests for `LibraryCache` in `robotcode-robot`.

use std::path::PathBuf;
use std::sync::Arc;

use robotcode_python_bridge::MockBridge;
use robotcode_robot::diagnostics::{LibraryCache, LibraryCacheKey};
use serde_json::json;

fn make_key(name: &str) -> LibraryCacheKey {
    LibraryCacheKey::new(name, vec![], vec![], PathBuf::from("python3"))
}

#[tokio::test]
async fn library_cache_returns_doc() {
    let bridge = MockBridge::with_responses([(
        "library_doc",
        vec![json!({
            "name": "BuiltIn",
            "doc": "Built-in library.",
            "version": "7.0.0",
            "scope": "GLOBAL",
            "named_args": true,
            "keywords": [],
            "inits": [],
            "typedocs": []
        })],
    )]);

    let cache = LibraryCache::new(Arc::new(bridge));
    let key = make_key("BuiltIn");

    let doc = cache.get(&key, None).await.unwrap();
    assert_eq!(doc.name, "BuiltIn");
}

#[tokio::test]
async fn library_cache_hit_does_not_call_bridge_again() {
    // Only one response registered; a second call would fail if the cache miss
    // path were taken again.
    let bridge = MockBridge::with_responses([(
        "library_doc",
        vec![json!({
            "name": "BuiltIn",
            "doc": "",
            "version": "7.0.0",
            "scope": "GLOBAL",
            "named_args": true,
            "keywords": [],
            "inits": [],
            "typedocs": []
        })],
    )]);

    let cache = LibraryCache::new(Arc::new(bridge));
    let key = make_key("BuiltIn");

    // First call — populates cache.
    let doc1 = cache.get(&key, None).await.unwrap();
    assert_eq!(cache.len(), 1);

    // Second call — must be served from cache (no bridge call).
    let doc2 = cache.get(&key, None).await.unwrap();
    assert_eq!(doc1.name, doc2.name);
    // Cache size must still be 1 (no duplicate insert).
    assert_eq!(cache.len(), 1);
}

#[tokio::test]
async fn library_cache_invalidate_clears_entry() {
    let bridge = MockBridge::with_responses([(
        "library_doc",
        vec![
            json!({"name":"BuiltIn","doc":"","version":"7.0.0","scope":"GLOBAL","named_args":true,"keywords":[],"inits":[],"typedocs":[]}),
            json!({"name":"BuiltIn","doc":"refreshed","version":"7.0.0","scope":"GLOBAL","named_args":true,"keywords":[],"inits":[],"typedocs":[]}),
        ],
    )]);

    let cache = LibraryCache::new(Arc::new(bridge));
    let key = make_key("BuiltIn");

    let doc1 = cache.get(&key, None).await.unwrap();
    assert_eq!(doc1.doc, "");

    cache.invalidate(&key);
    assert!(cache.is_empty());

    // Should fetch again — gets second response.
    let doc2 = cache.get(&key, None).await.unwrap();
    assert_eq!(doc2.doc, "refreshed");
}

#[tokio::test]
async fn library_cache_clear() {
    let bridge = MockBridge::with_responses([(
        "library_doc",
        vec![
            json!({"name":"BuiltIn","doc":"","version":"7.0.0","scope":"GLOBAL","named_args":true,"keywords":[],"inits":[],"typedocs":[]}),
            json!({"name":"Collections","doc":"","version":"7.0.0","scope":"GLOBAL","named_args":true,"keywords":[],"inits":[],"typedocs":[]}),
        ],
    )]);

    let cache = LibraryCache::new(Arc::new(bridge));

    cache.get(&make_key("BuiltIn"), None).await.unwrap();
    cache.get(&make_key("Collections"), None).await.unwrap();
    assert_eq!(cache.len(), 2);

    cache.clear();
    assert!(cache.is_empty());
}
