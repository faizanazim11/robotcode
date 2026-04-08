//! `RobotCodeServer` — implements the `tower_lsp::LanguageServer` trait.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use robotcode_jsonrpc2::lsp_types::*;
use robotcode_jsonrpc2::{async_trait, Client, LanguageServer, Result};
use tracing::info;

use robotcode_rf_parser::parser::parse;
use robotcode_robot::diagnostics::{DocumentCache, Namespace, NamespaceAnalyzer};

use crate::handlers::{
    self,
    semantic_tokens::{legend, semantic_tokens},
};

// ── Server ────────────────────────────────────────────────────────────────────

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
    /// Current text content of open documents (keyed by URI string).
    document_texts: Arc<DashMap<String, Arc<String>>>,
}

impl RobotCodeServer {
    /// Create a new server instance bound to `client`.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            python: None,
            document_cache: Arc::new(DocumentCache::new()),
            document_texts: Arc::new(DashMap::new()),
        }
    }

    /// Create a server with a specific Python interpreter path for the bridge.
    pub fn with_python(client: Client, python: Option<PathBuf>) -> Self {
        Self {
            client,
            python: python.map(Arc::new),
            document_cache: Arc::new(DocumentCache::new()),
            document_texts: Arc::new(DashMap::new()),
        }
    }

    /// Analyze a document and push diagnostics to the client.
    ///
    /// Parses `text` using the RF parser, runs the namespace analyzer, caches
    /// the result, and sends a `textDocument/publishDiagnostics` notification.
    async fn analyze_and_publish(&self, uri: Url, text: &str, version: Option<i32>) {
        let file = parse(text);

        // Build a minimal namespace (no import resolution yet — Phase 6 wires
        // in the Python bridge for library introspection in a later iteration).
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

    /// Return the current text for `uri` and its parsed AST.
    fn get_document_text(&self, uri: &Url) -> Option<Arc<String>> {
        self.document_texts
            .get(uri.as_str())
            .map(|e| Arc::clone(e.value()))
    }
}

#[async_trait]
impl LanguageServer for RobotCodeServer {
    // ── Lifecycle ─────────────────────────────────────────────────────────────

    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!(
            root_uri = ?params.root_uri,
            python = ?self.python.as_deref().map(|p| p.display().to_string()),
            "Received initialize request"
        );

        let semantic_tokens_provider = Some(
            SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                work_done_progress_options: Default::default(),
                legend: legend(),
                range: Some(false),
                full: Some(SemanticTokensFullOptions::Bool(true)),
            }),
        );

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // FULL sync: the analysis engine works on the complete document text.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // Phase 6 capabilities.
                semantic_tokens_provider,
                document_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(false),
                    work_done_progress_options: Default::default(),
                })),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "$".to_string(),
                        "{".to_string(),
                        " ".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
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

    // ── Text document sync ────────────────────────────────────────────────────

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = &params.text_document;
        handlers::text_document::did_open(&self.client, &params).await;

        // Store document text.
        self.document_texts
            .insert(doc.uri.as_str().to_owned(), Arc::new(doc.text.clone()));

        if doc.language_id == "robotframework"
            || doc.uri.as_str().ends_with(".robot")
            || doc.uri.as_str().ends_with(".resource")
        {
            self.analyze_and_publish(doc.uri.clone(), &doc.text, Some(doc.version))
                .await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        handlers::text_document::did_change(&self.client, &params).await;

        // With FULL sync, there is exactly one content_change containing the
        // whole document text.
        if let Some(change) = params.content_changes.into_iter().next() {
            let uri = params.text_document.uri;
            let version = params.text_document.version;

            // Update stored text.
            self.document_texts
                .insert(uri.as_str().to_owned(), Arc::new(change.text.clone()));

            if uri.as_str().ends_with(".robot") || uri.as_str().ends_with(".resource") {
                self.analyze_and_publish(uri, &change.text, Some(version))
                    .await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = &params.text_document.uri;
        handlers::text_document::did_close(&self.client, &params).await;

        // Remove stored text and cached diagnostics.
        self.document_texts.remove(uri.as_str());
        self.document_cache.remove(uri.as_str());
        self.client
            .publish_diagnostics(uri.clone(), vec![], None)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        handlers::text_document::did_save(&self.client, &params).await;
    }

    // ── Phase 6: Text Document Features ──────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let Some(text) = self.get_document_text(&params.text_document.uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let tokens = semantic_tokens(&file, &text);
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some(text) = self.get_document_text(&params.text_document.uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let symbols = handlers::document_symbols::document_symbols(&file);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let Some(text) = self.get_document_text(&params.text_document.uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        Ok(Some(handlers::folding_range::folding_ranges(&file)))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let Some(text) =
            self.get_document_text(&params.text_document_position_params.text_document.uri)
        else {
            return Ok(None);
        };
        let file = parse(&text);
        let pos = params.text_document_position_params.position;
        Ok(Some(handlers::highlight::document_highlight(
            &file, &text, pos,
        )))
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let Some(text) = self.get_document_text(&params.text_document.uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        Ok(Some(handlers::selection_range::selection_ranges(
            &file,
            params.positions,
        )))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let Some(text) = self.get_document_text(&params.text_document.uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let ns = Namespace::new(params.text_document.uri.to_file_path().ok());
        Ok(Some(handlers::inlay_hints::inlay_hints(
            &file,
            &ns,
            params.range,
        )))
    }

    // ── Phase 6: Navigation Features ──────────────────────────────────────────

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let ns = Namespace::new(uri.to_file_path().ok());
        let pos = params.text_document_position_params.position;
        Ok(handlers::goto::goto_definition(&file, &ns, uri, pos))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        let refs = handlers::references::references(&file, &text, uri, pos, include_decl);
        Ok(if refs.is_empty() { None } else { Some(refs) })
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let pos = params.text_document_position.position;
        Ok(handlers::rename::rename(
            &file,
            &text,
            uri,
            pos,
            params.new_name,
        ))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = &params.query;
        let mut all: Vec<SymbolInformation> = Vec::new();

        // Collect from all open documents.
        for entry in self.document_texts.iter() {
            let uri_str = entry.key().clone();
            let text = entry.value().clone();
            if let Ok(uri) = Url::parse(&uri_str) {
                let file = parse(&text);
                let syms = handlers::workspace_symbols::workspace_symbols(&file, &uri, query);
                all.extend(syms);
            }
        }

        Ok(if all.is_empty() { None } else { Some(all) })
    }

    // ── Phase 6: Completion & Hints ───────────────────────────────────────────

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let ns = Namespace::new(uri.to_file_path().ok());
        let pos = params.text_document_position.position;
        let items = handlers::completion::completions(&file, &ns, pos);
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let ns = Namespace::new(uri.to_file_path().ok());
        let pos = params.text_document_position_params.position;
        Ok(handlers::hover::hover(&file, &text, &ns, pos))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let ns = Namespace::new(uri.to_file_path().ok());
        let pos = params.text_document_position_params.position;
        Ok(handlers::signature_help::signature_help(&file, &ns, pos))
    }

    // ── Phase 6: Code Actions & Formatting ───────────────────────────────────

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let ns = Namespace::new(uri.to_file_path().ok());
        let actions = handlers::code_actions::code_actions(
            &file,
            &ns,
            uri,
            params.range,
            params.context.diagnostics,
        );
        Ok(if actions.is_empty() {
            None
        } else {
            Some(actions)
        })
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = &params.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        let file = parse(&text);
        let lenses = handlers::code_lens::code_lens(&file, uri);
        Ok(if lenses.is_empty() {
            None
        } else {
            Some(lenses)
        })
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        let Some(text) = self.get_document_text(uri) else {
            return Ok(None);
        };
        Ok(handlers::formatting::format_document(&text))
    }
}
