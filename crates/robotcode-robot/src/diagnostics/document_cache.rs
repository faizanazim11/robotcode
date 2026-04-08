//! Per-document analysis cache.
//!
//! [`DocumentCache`] stores the analysis result (diagnostics + namespace) for
//! each open document.  Results are invalidated when the document changes or
//! when an imported file changes (invalidation cascade).
//!
//! Mirrors the structure of `robotcode.robot.diagnostics.document_cache_helper`.

use std::sync::Arc;

use dashmap::DashMap;
use lsp_types::Diagnostic;
use tokio::sync::RwLock;
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// DocumentAnalysis
// ---------------------------------------------------------------------------

/// The cached analysis output for a single document.
#[derive(Debug, Clone, Default)]
pub struct DocumentAnalysis {
    /// Diagnostics produced by the last analysis pass.
    pub diagnostics: Vec<Diagnostic>,
    /// The document version at the time of analysis (from the LSP protocol).
    pub version: Option<i32>,
    /// Whether this result is still valid or has been invalidated.
    pub valid: bool,
}

impl DocumentAnalysis {
    fn new(diagnostics: Vec<Diagnostic>, version: Option<i32>) -> Self {
        Self {
            diagnostics,
            version,
            valid: true,
        }
    }
}

// ---------------------------------------------------------------------------
// DocumentCache
// ---------------------------------------------------------------------------

/// Thread-safe per-document analysis cache.
///
/// Each document URI maps to an `Arc<RwLock<Option<DocumentAnalysis>>>`.
/// - `None` means the document has not been analyzed yet (or was invalidated).
/// - `Some(analysis)` holds the most recent analysis result.
///
/// Concurrent reads are allowed; writes acquire an exclusive lock per
/// document without blocking unrelated documents.
pub struct DocumentCache {
    /// Maps document URI strings to their cached analysis.
    entries: DashMap<String, Arc<RwLock<Option<DocumentAnalysis>>>>,
}

impl DocumentCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Store an analysis result for `uri`.
    pub async fn store(
        &self,
        uri: &str,
        diagnostics: Vec<Diagnostic>,
        version: Option<i32>,
    ) -> Arc<RwLock<Option<DocumentAnalysis>>> {
        let slot = self
            .entries
            .entry(uri.to_owned())
            .or_insert_with(|| Arc::new(RwLock::new(None)))
            .clone();

        let mut guard = slot.write().await;
        info!(uri, "Storing analysis result");
        *guard = Some(DocumentAnalysis::new(diagnostics, version));
        drop(guard);
        slot
    }

    /// Return the cached analysis for `uri`, if any.
    pub async fn get(&self, uri: &str) -> Option<DocumentAnalysis> {
        let slot = self.entries.get(uri)?.clone();
        let guard = slot.read().await;
        guard.clone()
    }

    /// Return the cached diagnostics for `uri`, if any.
    pub async fn get_diagnostics(&self, uri: &str) -> Option<Vec<Diagnostic>> {
        let analysis = self.get(uri).await?;
        if analysis.valid {
            Some(analysis.diagnostics)
        } else {
            None
        }
    }

    /// Invalidate the cached analysis for `uri`.
    ///
    /// The next call to [`store`] will replace the stale entry.
    pub async fn invalidate(&self, uri: &str) {
        if let Some(slot) = self.entries.get(uri) {
            let mut guard = slot.write().await;
            if let Some(analysis) = guard.as_mut() {
                debug!(uri, "Invalidating cached analysis");
                analysis.valid = false;
            }
        }
    }

    /// Remove the entry for `uri` entirely.
    pub fn remove(&self, uri: &str) {
        self.entries.remove(uri);
    }

    /// Remove all cached entries.
    pub fn clear(&self) {
        self.entries.clear();
    }

    /// Return the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_get() {
        let cache = DocumentCache::new();
        let uri = "file:///test.robot";
        cache.store(uri, vec![], Some(1)).await;
        let analysis = cache.get(uri).await.unwrap();
        assert_eq!(analysis.version, Some(1));
        assert!(analysis.valid);
    }

    #[tokio::test]
    async fn test_invalidate() {
        let cache = DocumentCache::new();
        let uri = "file:///test.robot";
        cache.store(uri, vec![], Some(1)).await;
        cache.invalidate(uri).await;
        let diags = cache.get_diagnostics(uri).await;
        assert!(
            diags.is_none(),
            "Invalidated entry should not return diagnostics"
        );
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = DocumentCache::new();
        let uri = "file:///test.robot";
        cache.store(uri, vec![], None).await;
        assert_eq!(cache.len(), 1);
        cache.remove(uri);
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn test_get_missing() {
        let cache = DocumentCache::new();
        assert!(cache.get("file:///missing.robot").await.is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = DocumentCache::new();
        cache.store("file:///a.robot", vec![], None).await;
        cache.store("file:///b.robot", vec![], None).await;
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }
}
