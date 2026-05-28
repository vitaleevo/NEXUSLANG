use std::sync::{Arc, Mutex};

use nexus_lsp::{semantic_tokens_legend, LspCore};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    core: Arc<Mutex<LspCore>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: None,
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    completion_item: None,
                }),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                            legend: semantic_tokens_legend(),
                            range: None,
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "nexus-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "nexus-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = Some(params.text_document.version);
        let text = params.text_document.text;
        {
            let mut core = self.core.lock().unwrap();
            core.open_document(uri.clone(), version, text);
        }
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = Some(params.text_document.version);
        if let Some(change) = params.content_changes.into_iter().last() {
            let mut core = self.core.lock().unwrap();
            core.change_document(uri.clone(), version, change.text);
        }
        self.publish_diagnostics(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let batches = {
            let mut core = self.core.lock().unwrap();
            core.close_document_publish_batches(&params.text_document.uri)
        };
        for batch in batches {
            self.client
                .publish_diagnostics(batch.uri, batch.diagnostics, batch.version)
                .await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let hover = {
            let core = self.core.lock().unwrap();
            core.hover(&uri, pos)
        };
        Ok(hover)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let completion = {
            let core = self.core.lock().unwrap();
            core.completion(&uri)
        };
        Ok(completion)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let definition = {
            let core = self.core.lock().unwrap();
            core.goto_definition(&uri, pos)
        };
        Ok(definition.map(GotoDefinitionResponse::Scalar))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let tokens = {
            let core = self.core.lock().unwrap();
            core.semantic_tokens(&uri)
        };
        Ok(tokens.map(SemanticTokensResult::Tokens))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let symbols = {
            let core = self.core.lock().unwrap();
            core.document_symbols(&uri)
        };
        Ok(symbols)
    }
}

impl Backend {
    async fn publish_diagnostics(&self, uri: Url) {
        let batches = {
            let mut core = self.core.lock().unwrap();
            core.diagnostic_publish_batches_for(&uri)
        };
        if let Some(batches) = batches {
            for batch in batches {
                self.client
                    .publish_diagnostics(batch.uri, batch.diagnostics, batch.version)
                    .await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let core = Arc::new(Mutex::new(LspCore::new()));

    let (service, socket) = LspService::new(|client| Backend {
        client,
        core: core.clone(),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
