use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use ast_grep_config::Configs;
use ast_grep_config::Severity;
use ast_grep_core::{language::Language, AstGrep, NodeMatch};

#[derive(Clone)]
struct VersionedAst<L: Language> {
    version: i32,
    root: AstGrep<L>,
}

struct Backend<L: Language> {
    client: Client,
    map: DashMap<String, VersionedAst<L>>,
    configs: Configs<L>,
    language: L,
}

#[tower_lsp::async_trait]
impl<L: Language + Send + Sync + 'static> LanguageServer for Backend<L> {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "ast-grep language server".to_string(),
                version: None,
            }),
            capabilities: ServerCapabilities {
                // TODO: change this to incremental
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                code_action_provider: None,
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.on_open(params).await
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.on_change(params).await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.on_close(params).await;
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }
}

fn convert_node_match_to_range<L: Language>(node_match: NodeMatch<L>) -> Range {
    let (start_row, start_col) = node_match.start_pos();
    let (end_row, end_col) = node_match.end_pos();
    Range {
        start: Position {
            line: start_row as u32,
            character: start_col as u32,
        },
        end: Position {
            line: end_row as u32,
            character: end_col as u32,
        },
    }
}

fn url_to_code_description(url: &Option<String>) -> Option<CodeDescription> {
    let href = Url::parse(url.as_ref()?).ok()?;
    Some(CodeDescription { href })
}

impl<L: Language> Backend<L> {
    async fn publish_diagnostics(&self, uri: Url, versioned: &VersionedAst<L>) {
        let mut diagnostics = vec![];
        for config in &self.configs.configs {
            let matcher = config.get_matcher();
            // TODO: don't run rules with unmatching language
            diagnostics.extend(
                versioned
                    .root
                    .root()
                    .find_all(&matcher)
                    .map(|m| Diagnostic {
                        range: convert_node_match_to_range(m),
                        code: Some(NumberOrString::String(config.id.clone())),
                        code_description: url_to_code_description(&config.url),
                        severity: Some(match config.severity {
                            Severity::Error => DiagnosticSeverity::ERROR,
                            Severity::Warning => DiagnosticSeverity::WARNING,
                            Severity::Info => DiagnosticSeverity::INFORMATION,
                            Severity::Hint => DiagnosticSeverity::HINT,
                        }),
                        message: config.message.clone(),
                        source: Some(String::from("ast-grep")),
                        tags: None,
                        related_information: None, // TODO: add labels
                        data: None,
                    }),
            );
        }
        self.client
            .publish_diagnostics(uri, diagnostics, Some(versioned.version))
            .await;
    }
    async fn on_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.as_str().to_string();
        let text = params.text_document.text;
        let root = AstGrep::new(text, self.language.clone());
        let versioned = VersionedAst {
            version: params.text_document.version,
            root,
        };
        let copied = versioned.clone();
        self.map.insert(uri, versioned); // don't lock dashmap
        self.publish_diagnostics(params.text_document.uri, &copied)
            .await;
    }
    async fn on_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.as_str();
        let text = &params.content_changes[0].text;
        let root = AstGrep::new(text, self.language.clone());
        let mut versioned = match self.map.get_mut(uri) {
            Some(ast) => ast,
            None => return,
        };
        // skip old version update
        if versioned.version > params.text_document.version {
            return;
        }
        *versioned = VersionedAst {
            version: params.text_document.version,
            root,
        };
        let copied = versioned.clone();
        drop(versioned); // don't lock dashmap
        self.publish_diagnostics(params.text_document.uri, &copied)
            .await;
    }
    async fn on_close(&self, params: DidCloseTextDocumentParams) {
        self.map.remove(params.text_document.uri.as_str());
    }
}
