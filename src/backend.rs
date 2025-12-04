use std::path::PathBuf;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::completion;
use crate::diagnostics;
use crate::hover;
use crate::semantic_tokens;
use crate::state::ServerState;
use crate::symbols;

pub struct Backend {
    client: Client,
    state: ServerState,
    /// Workspace root path for loading configuration files
    workspace_root: std::sync::RwLock<Option<PathBuf>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: ServerState::new(),
            workspace_root: std::sync::RwLock::new(None),
        }
    }

    /// Try to load aisle.conf from the workspace
    fn load_aisle_config(&self) {
        if let Ok(guard) = self.workspace_root.read() {
            if let Some(ref path) = *guard {
                self.state.load_aisle_config(path);
            }
        }
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let diagnostics = if let Some(doc) = self.state.get_document(uri) {
            diagnostics::get_diagnostics(&doc)
        } else {
            vec![]
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract workspace root from initialization params
        let workspace_path = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|folder| folder.uri.to_file_path().ok())
            .or_else(|| {
                #[allow(deprecated)]
                params.root_uri.as_ref().and_then(|uri| uri.to_file_path().ok())
            })
            .or_else(|| {
                #[allow(deprecated)]
                params.root_path.as_ref().map(PathBuf::from)
            });

        if let Some(path) = workspace_path {
            tracing::info!("Workspace root: {:?}", path);
            if let Ok(mut guard) = self.workspace_root.write() {
                *guard = Some(path);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
                        })),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "@".into(),
                        "#".into(),
                        "~".into(),
                        "%".into(),
                        "{".into(),
                    ]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(semantic_tokens::capabilities()),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "cooklang-language-server".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("Cooklang LSP initialized");

        // Load aisle.conf if available in workspace
        self.load_aisle_config();

        self.client
            .log_message(MessageType::INFO, "Cooklang Language Server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Cooklang LSP shutting down");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let content = params.text_document.text;

        tracing::debug!("Document opened: {}", uri);
        self.state.open_document(uri.clone(), version, content);
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        if let Some(change) = params.content_changes.into_iter().last() {
            tracing::debug!("Document changed: {}", uri);
            self.state.update_document(&uri, version, change.text);
            self.publish_diagnostics(&uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        tracing::debug!("Document saved: {}", params.text_document.uri);
        self.publish_diagnostics(&params.text_document.uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        tracing::debug!("Document closed: {}", uri);
        self.state.close_document(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;

        let response = if let Some(doc) = self.state.get_document(uri) {
            completion::get_completions(&doc, &params, &self.state)
        } else {
            None
        };

        Ok(response)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;

        let response = if let Some(doc) = self.state.get_document(uri) {
            hover::get_hover(&doc, &params)
        } else {
            None
        };

        Ok(response)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        let response = if let Some(doc) = self.state.get_document(uri) {
            symbols::get_document_symbols(&doc)
        } else {
            None
        };

        Ok(response)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;

        let tokens = if let Some(doc) = self.state.get_document(uri) {
            semantic_tokens::get_semantic_tokens(&doc)
        } else {
            vec![]
        };

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }
}
