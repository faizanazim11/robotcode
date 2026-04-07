//! `RobotCodeServer` — implements the `tower_lsp::LanguageServer` trait.

use std::path::PathBuf;
use std::sync::Arc;

use robotcode_jsonrpc2::lsp_types::*;
use robotcode_jsonrpc2::{async_trait, Client, LanguageServer, Result};
use tracing::info;

use robotcode_rf_parser::parser::parse;
use robotcode_robot::diagnostics::{DocumentCache, Namespace, NamespaceAnalyzer};

use crate::handlers::text_document;

/// The main language server struct.
///
/// Holds a `Client` handle (used to push notifications to the editor) and will
/// eventually hold per-workspace state, document caches, etc.
pub struct RobotCodeServer {
    client: Client,
    /// Path to the Python interpreter for the Python bridge (if configured).
    python: Option<Arc<PathBuf>>,
    /// Per-document analysis result cache.
    document_cache: Arc<DocumentCache>,
}

impl RobotCodeServer {
    /// Create a new server instance bound to `client`.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            python: None,
            document_cache: Arc::new(DocumentCache::new()),
        }
    }

    /// Create a server with a specific Python interpreter path for the bridge.
    pub fn with_python(client: Client, python: Option<PathBuf>) -> Self {
        Self {
            client,
            python: python.map(Arc::new),
            document_cache: Arc::new(DocumentCache::new()),
        }
    }

    /// Analyze a document and push diagnostics to the client.
    ///
    /// Parses `text` using the RF parser, runs the namespace analyzer, caches
    /// the result, and sends a `textDocument/publishDiagnostics` notification.
    async fn analyze_and_publish(&self, uri: Url, text: &str, version: Option<i32>) {
        let file = parse(text);

        // Build a minimal namespace (no import resolution yet — Phase 6 will
        // wire in the Python bridge for library introspection).
        let ns = Namespace::new(uri.to_file_path().ok());

        let analyzer = NamespaceAnalyzer::new(&ns);
        let result = analyzer.analyze(&file);
        let diagnostics = result.diagnostics;

        // Cache the result.
        self.document_cache
            .store(uri.as_str(), diagnostics.clone(), version)
            .await;

        // Publish to the client.
        self.client
            .publish_diagnostics(uri, diagnostics, version)
            .await;
    }
}

#[async_trait]
impl LanguageServer for RobotCodeServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!(
            root_uri = ?params.root_uri,
            python = ?self.python.as_deref().map(|p| p.display().to_string()),
            "Received initialize request"
        );
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // FULL sync is required because the diagnostics engine in Phase 5
                // works on the complete document text for each analysis pass.
                // Incremental sync will be supported once the parser gains an
                // incremental update API in a later phase.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "robotcode".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        info!("RobotCode language server initialized");
        self.client
            .log_message(MessageType::INFO, "RobotCode language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutdown requested");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = &params.text_document;
        text_document::did_open(&self.client, &params).await;
        if doc.language_id == "robotframework"
            || doc.uri.as_str().ends_with(".robot")
            || doc.uri.as_str().ends_with(".resource")
        {
            self.analyze_and_publish(doc.uri.clone(), &doc.text, Some(doc.version))
                .await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        text_document::did_change(&self.client, &params).await;
        // With FULL sync, there is exactly one content_change containing the
        // whole document text.
        if let Some(change) = params.content_changes.into_iter().next() {
            let uri = params.text_document.uri;
            let version = params.text_document.version;
            if uri.as_str().ends_with(".robot") || uri.as_str().ends_with(".resource") {
                self.analyze_and_publish(uri, &change.text, Some(version))
                    .await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = &params.text_document.uri;
        text_document::did_close(&self.client, &params).await;
        // Clear cached diagnostics and publish empty list to clear editor gutter.
        self.document_cache.remove(uri.as_str());
        self.client
            .publish_diagnostics(uri.clone(), vec![], None)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        text_document::did_save(&self.client, &params).await;
    }
}
