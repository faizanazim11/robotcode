//! Handlers for `textDocument/*` LSP notifications.

use robotcode_jsonrpc2::lsp_types::*;
use robotcode_jsonrpc2::Client;
use tracing::{debug, info};

/// Handle `textDocument/didOpen`.
pub async fn did_open(client: &Client, params: &DidOpenTextDocumentParams) {
    let doc = &params.text_document;
    info!(
        uri = %doc.uri,
        language_id = %doc.language_id,
        version = doc.version,
        "Document opened"
    );
    client
        .log_message(MessageType::LOG, format!("Opened: {}", doc.uri))
        .await;
}

/// Handle `textDocument/didChange`.
pub async fn did_change(client: &Client, params: &DidChangeTextDocumentParams) {
    let uri = &params.text_document.uri;
    let version = params.text_document.version;
    debug!(
        %uri,
        version,
        changes = params.content_changes.len(),
        "Document changed"
    );
    client
        .log_message(
            MessageType::LOG,
            format!("Changed: {} (version {})", uri, version),
        )
        .await;
}

/// Handle `textDocument/didClose`.
pub async fn did_close(client: &Client, params: &DidCloseTextDocumentParams) {
    let uri = &params.text_document.uri;
    info!(%uri, "Document closed");
    client
        .log_message(MessageType::LOG, format!("Closed: {}", uri))
        .await;
}

/// Handle `textDocument/didSave`.
pub async fn did_save(client: &Client, params: &DidSaveTextDocumentParams) {
    let uri = &params.text_document.uri;
    info!(%uri, "Document saved");
    client
        .log_message(MessageType::LOG, format!("Saved: {}", uri))
        .await;
}
