//! Thread-safe open document registry.
//!
//! Port of the Python `robotcode.core.documents_manager` module.
//! Uses [`DashMap`] for lock-free concurrent access.

use std::sync::Arc;

use dashmap::DashMap;

use crate::text_document::{SharedTextDocument, TextDocument, TextDocumentContentChangeEvent};
use crate::uri::Uri;

/// Error type for document operations.
#[derive(Debug, thiserror::Error)]
pub enum DocumentsManagerError {
    #[error("Document not found: {0}")]
    NotFound(String),
    #[error("URI error: {0}")]
    Uri(#[from] crate::uri::UriError),
    #[error("Text document error: {0}")]
    TextDocument(#[from] crate::text_document::TextDocumentError),
}

/// Thread-safe registry of currently open [`TextDocument`]s.
///
/// All keys are normalized URI strings.
#[derive(Debug, Default)]
pub struct DocumentsManager {
    documents: DashMap<String, SharedTextDocument>,
}

impl DocumentsManager {
    /// Create a new, empty documents manager.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            documents: DashMap::new(),
        })
    }

    /// Return the number of currently tracked documents.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Return `true` if no documents are tracked.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Open (or re-open) a document.
    ///
    /// If a document with the same URI already exists it is replaced.
    pub fn open(
        &self,
        document_uri: &str,
        text: &str,
        language_id: Option<String>,
        version: Option<i32>,
    ) -> Result<SharedTextDocument, DocumentsManagerError> {
        let doc = Arc::new(TextDocument::new(document_uri, text, language_id, version)?);
        let key = doc.uri.to_string();
        self.documents.insert(key, Arc::clone(&doc));
        Ok(doc)
    }

    /// Get a document by URI (normalized).
    pub fn get(&self, uri: &str) -> Option<SharedTextDocument> {
        let normalized = Uri::parse(uri).ok()?.normalized().to_string();
        self.documents.get(&normalized).map(|v| Arc::clone(&*v))
    }

    /// Apply incremental changes to an open document.
    pub fn change(
        &self,
        uri: &str,
        version: Option<i32>,
        changes: &[TextDocumentContentChangeEvent],
    ) -> Result<SharedTextDocument, DocumentsManagerError> {
        let normalized = Uri::parse(uri)
            .map_err(DocumentsManagerError::Uri)?
            .normalized()
            .to_string();

        let doc = self
            .documents
            .get(&normalized)
            .ok_or_else(|| DocumentsManagerError::NotFound(normalized.clone()))?;

        doc.apply_changes(version, changes);
        Ok(Arc::clone(&*doc))
    }

    /// Close a document and remove it from the registry.
    pub fn close(&self, uri: &str) -> Option<SharedTextDocument> {
        let normalized = Uri::parse(uri).ok()?.normalized().to_string();
        self.documents.remove(&normalized).map(|(_, v)| v)
    }

    /// Iterate over all currently open documents.
    pub fn iter(&self) -> impl Iterator<Item = SharedTextDocument> + '_ {
        self.documents.iter().map(|entry| Arc::clone(entry.value()))
    }

    /// Return all currently open URIs.
    pub fn uris(&self) -> Vec<String> {
        self.documents.iter().map(|e| e.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_document::Range;
    use lsp_types::Position;

    #[test]
    fn test_open_and_get() {
        let mgr = DocumentsManager::new();
        mgr.open(
            "file:///test.robot",
            "*** Test Cases ***\n",
            Some("robotframework".into()),
            Some(1),
        )
        .unwrap();

        let doc = mgr.get("file:///test.robot").unwrap();
        assert_eq!(doc.version(), Some(1));
        assert_eq!(doc.text(), "*** Test Cases ***\n");
    }

    #[test]
    fn test_change() {
        let mgr = DocumentsManager::new();
        mgr.open("file:///test.robot", "hello", None, Some(1))
            .unwrap();

        let changes = vec![TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            }),
            range_length: None,
            text: "world".to_string(),
        }];

        mgr.change("file:///test.robot", Some(2), &changes).unwrap();
        let doc = mgr.get("file:///test.robot").unwrap();
        assert_eq!(doc.text(), "world");
        assert_eq!(doc.version(), Some(2));
    }

    #[test]
    fn test_close() {
        let mgr = DocumentsManager::new();
        mgr.open("file:///test.robot", "content", None, None)
            .unwrap();
        assert_eq!(mgr.len(), 1);
        mgr.close("file:///test.robot");
        assert_eq!(mgr.len(), 0);
    }
}
