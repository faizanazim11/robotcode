//! `RobotCodeServer` — implements the `tower_lsp::LanguageServer` trait.

use robotcode_jsonrpc2::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tracing::info;

use crate::handlers::text_document;

/// The main language server struct.
///
/// Holds a `Client` handle (used to push notifications to the editor) and will
/// eventually hold per-workspace state, document caches, etc.
pub struct RobotCodeServer {
    client: Client,
}

impl RobotCodeServer {
    /// Create a new server instance bound to `client`.
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for RobotCodeServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!(
            root_uri = ?params.root_uri,
            "Received initialize request"
        );
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
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
        text_document::did_open(&self.client, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        text_document::did_change(&self.client, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        text_document::did_close(&self.client, params).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        text_document::did_save(&self.client, params).await;
    }
}
