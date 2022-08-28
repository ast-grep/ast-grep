#![feature(trait_alias)]

use std::hash::Hash;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use ast_grep_config::RuleCollection;
use ast_grep_config::Severity;
use ast_grep_core::{language::Language, AstGrep, NodeMatch};

pub use tower_lsp::{LspService, Server};

pub trait LSPLang = Language + Hash + Eq + Send + Sync + 'static;

#[derive(Clone)]
struct VersionedAst<L: Language> {
    version: i32,
    root: AstGrep<L>,
}

pub struct Backend<L: LSPLang> {
    client: Client,
    map: DashMap<String, VersionedAst<L>>,
    rules: RuleCollection<L>,
}

#[tower_lsp::async_trait]
impl<L: LSPLang> LanguageServer for Backend<L> {
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
        self.on_open(params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.on_change(params).await;
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

impl<L: LSPLang> Backend<L> {
    pub fn new(client: Client, rules: RuleCollection<L>) -> Self {
        Self {
            client,
            rules,
            map: DashMap::new(),
        }
    }
    async fn publish_diagnostics(&self, uri: Url, versioned: &VersionedAst<L>) -> Option<()> {
        let mut diagnostics = vec![];
        let lang = Self::infer_lang_from_uri(&uri)?;
        let rules = self.rules.get_rules_for_lang(&lang);
        for rule in rules {
            let matcher = rule.get_matcher();
            // TODO: don't run rules with unmatching language
            diagnostics.extend(
                versioned
                    .root
                    .root()
                    .find_all(&matcher)
                    .map(|m| Diagnostic {
                        range: convert_node_match_to_range(m),
                        code: Some(NumberOrString::String(rule.id.clone())),
                        code_description: url_to_code_description(&rule.url),
                        severity: Some(match rule.severity {
                            Severity::Error => DiagnosticSeverity::ERROR,
                            Severity::Warning => DiagnosticSeverity::WARNING,
                            Severity::Info => DiagnosticSeverity::INFORMATION,
                            Severity::Hint => DiagnosticSeverity::HINT,
                        }),
                        message: rule.message.clone(),
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
        Some(())
    }
    async fn on_open(&self, params: DidOpenTextDocumentParams) -> Option<()> {
        let text_doc = params.text_document;
        let uri = text_doc.uri.as_str();
        let text = text_doc.text;
        let lang = Self::infer_lang_from_uri(&text_doc.uri)?;
        let root = AstGrep::new(text, lang);
        let versioned = VersionedAst {
            version: text_doc.version,
            root,
        };
        let copied = versioned.clone();
        self.map.insert(uri.to_owned(), versioned); // don't lock dashmap
        self.publish_diagnostics(text_doc.uri, &copied).await;
        Some(())
    }
    async fn on_change(&self, params: DidChangeTextDocumentParams) -> Option<()> {
        let text_doc = params.text_document;
        let uri = text_doc.uri.as_str();
        let text = &params.content_changes[0].text;
        let lang = Self::infer_lang_from_uri(&text_doc.uri)?;
        let root = AstGrep::new(text, lang);
        let mut versioned = self.map.get_mut(uri)?;
        // skip old version update
        if versioned.version > text_doc.version {
            return None;
        }
        *versioned = VersionedAst {
            version: text_doc.version,
            root,
        };
        let copied = versioned.clone();
        drop(versioned); // don't lock dashmap
        self.publish_diagnostics(text_doc.uri, &copied).await;
        Some(())
    }
    async fn on_close(&self, params: DidCloseTextDocumentParams) {
        self.map.remove(params.text_document.uri.as_str());
    }

    // TODO: support other urls besides file_scheme
    fn infer_lang_from_uri(uri: &Url) -> Option<L> {
        let path = uri.to_file_path().ok()?;
        L::from_path(path)
    }
}
