use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use ast_grep_config::Severity;
use ast_grep_config::{RuleCollection, RuleConfig};
use ast_grep_core::{language::Language, AstGrep, Doc, Node, NodeMatch, StrDoc};

use std::collections::HashMap;

pub use tower_lsp::{LspService, Server};

pub trait LSPLang: Language + Eq + Send + Sync + 'static {}
impl<T> LSPLang for T where T: Language + Eq + Send + Sync + 'static {}

struct VersionedAst<D: Doc> {
  version: i32,
  root: AstGrep<D>,
}

pub struct Backend<L: LSPLang> {
  client: Client,
  map: DashMap<String, VersionedAst<StrDoc<L>>>,
  rules: RuleCollection<L>,
}

#[derive(Serialize, Deserialize)]
pub struct MatchRequest {
  pattern: String,
}

#[derive(Serialize, Deserialize)]
pub struct MatchResult {
  uri: String,
  position: Range,
  content: String,
}

impl MatchResult {
  fn new(uri: String, position: Range, content: String) -> Self {
    Self {
      uri,
      position,
      content,
    }
  }
}

impl<L: LSPLang> Backend<L> {
  pub async fn search(&self, params: MatchRequest) -> Result<Vec<MatchResult>> {
    let matcher = params.pattern;
    let mut match_result = vec![];
    for slot in self.map.iter() {
      let uri = slot.key();
      let versioned = slot.value();
      for matched_node in versioned.root.root().find_all(matcher.as_str()) {
        let content = matched_node.text().to_string();
        let range = convert_node_to_range(&matched_node);
        match_result.push(MatchResult::new(uri.clone(), range, content));
      }
    }
    Ok(match_result)
  }
}

const FALLBAKC_CODE_ACTION_PROVIDER: Option<CodeActionProviderCapability> =
  Some(CodeActionProviderCapability::Simple(true));
fn code_action_provider(
  client_capability: &ClientCapabilities,
) -> Option<CodeActionProviderCapability> {
  let is_literal_supported = client_capability
    .text_document
    .as_ref()?
    .code_action
    .as_ref()?
    .code_action_literal_support
    .is_some();
  if !is_literal_supported {
    return None;
  }
  Some(CodeActionProviderCapability::Options(CodeActionOptions {
    code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
    work_done_progress_options: Default::default(),
    resolve_provider: Some(true),
  }))
}

#[tower_lsp::async_trait]
impl<L: LSPLang> LanguageServer for Backend<L> {
  async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
      server_info: Some(ServerInfo {
        name: "ast-grep language server".to_string(),
        version: None,
      }),
      capabilities: ServerCapabilities {
        // TODO: change this to incremental
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        code_action_provider: code_action_provider(&params.capabilities)
          .or(FALLBAKC_CODE_ACTION_PROVIDER),
        ..ServerCapabilities::default()
      },
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    self
      .client
      .log_message(MessageType::INFO, "server initialized!")
      .await;
  }

  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }

  async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
    self
      .client
      .log_message(MessageType::INFO, "workspace folders changed!")
      .await;
  }

  async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
    self
      .client
      .log_message(MessageType::INFO, "configuration changed!")
      .await;
  }

  async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
    self
      .client
      .log_message(MessageType::INFO, "watched files have changed!")
      .await;
  }
  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    self
      .client
      .log_message(MessageType::INFO, "file opened!")
      .await;
    self.on_open(params).await;
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    self.on_change(params).await;
  }

  async fn did_save(&self, _: DidSaveTextDocumentParams) {
    self
      .client
      .log_message(MessageType::INFO, "file saved!")
      .await;
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    self.on_close(params).await;
    self
      .client
      .log_message(MessageType::INFO, "file closed!")
      .await;
  }

  async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
    self
      .client
      .log_message(MessageType::INFO, "run code action!")
      .await;
    Ok(self.on_code_action(params).await)
  }
}

fn convert_node_to_range<D: Doc>(node_match: &Node<D>) -> Range {
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

fn convert_match_to_diagnostic<L: Language>(
  node_match: NodeMatch<StrDoc<L>>,
  rule: &RuleConfig<L>,
  uri: &Url,
) -> Diagnostic {
  Diagnostic {
    range: convert_node_to_range(&node_match),
    code: Some(NumberOrString::String(rule.id.clone())),
    code_description: url_to_code_description(&rule.url),
    severity: Some(match rule.severity {
      Severity::Error => DiagnosticSeverity::ERROR,
      Severity::Warning => DiagnosticSeverity::WARNING,
      Severity::Info => DiagnosticSeverity::INFORMATION,
      Severity::Hint => DiagnosticSeverity::HINT,
      Severity::Off => unreachable!("turned-off rule should not have match"),
    }),
    message: rule.get_message(&node_match),
    source: Some(String::from("ast-grep")),
    tags: None,
    related_information: collect_labels(&node_match, uri),
    data: None,
  }
}

fn collect_labels<L: Language>(
  node_match: &NodeMatch<StrDoc<L>>,
  uri: &Url,
) -> Option<Vec<DiagnosticRelatedInformation>> {
  let secondary_nodes = node_match.get_env().get_labels("secondary")?;
  Some(
    secondary_nodes
      .iter()
      .map(|n| {
        let location = Location {
          uri: uri.clone(),
          range: convert_node_to_range(n),
        };
        DiagnosticRelatedInformation {
          location,
          message: String::new(),
        }
      })
      .collect(),
  )
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
  async fn publish_diagnostics(&self, uri: Url, versioned: &VersionedAst<StrDoc<L>>) -> Option<()> {
    let mut diagnostics = vec![];
    let path = uri.to_file_path().ok()?;
    let rules = self.rules.for_path(&path);
    for rule in rules {
      let to_diagnostic = |m| convert_match_to_diagnostic(m, rule, &uri);
      let matcher = &rule.matcher;
      diagnostics.extend(versioned.root.root().find_all(matcher).map(to_diagnostic));
    }
    self
      .client
      .publish_diagnostics(uri, diagnostics, Some(versioned.version))
      .await;
    Some(())
  }
  async fn on_open(&self, params: DidOpenTextDocumentParams) -> Option<()> {
    let text_doc = params.text_document;
    let uri = text_doc.uri.as_str().to_owned();
    let text = text_doc.text;
    self
      .client
      .log_message(MessageType::LOG, "Parsing doc.")
      .await;
    let lang = Self::infer_lang_from_uri(&text_doc.uri)?;
    let root = AstGrep::new(text, lang);
    let versioned = VersionedAst {
      version: text_doc.version,
      root,
    };
    self
      .client
      .log_message(MessageType::LOG, "Publishing init diagnostics.")
      .await;
    self.publish_diagnostics(text_doc.uri, &versioned).await;
    self.map.insert(uri.to_owned(), versioned); // don't lock dashmap
    Some(())
  }
  async fn on_change(&self, params: DidChangeTextDocumentParams) -> Option<()> {
    let text_doc = params.text_document;
    let uri = text_doc.uri.as_str();
    let text = &params.content_changes[0].text;
    self
      .client
      .log_message(MessageType::LOG, "Parsing changed doc.")
      .await;
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
    self
      .client
      .log_message(MessageType::LOG, "Publishing diagnostics.")
      .await;
    self.publish_diagnostics(text_doc.uri, &versioned).await;
    Some(())
  }
  async fn on_close(&self, params: DidCloseTextDocumentParams) {
    self.map.remove(params.text_document.uri.as_str());
  }

  async fn on_code_action(&self, params: CodeActionParams) -> Option<CodeActionResponse> {
    let text_doc = params.text_document;
    let uri = text_doc.uri.as_str();
    let path = text_doc.uri.to_file_path().ok()?;
    let diagnostics = params.context.diagnostics;
    let error_id_to_ranges = Self::build_error_id_to_ranges(diagnostics);
    let versioned = self.map.get(uri)?;
    let mut response = CodeActionResponse::new();
    for config in self.rules.for_path(&path) {
      let ranges = match error_id_to_ranges.get(&config.id) {
        Some(ranges) => ranges,
        None => continue,
      };
      let matcher = &config.matcher;
      for matched_node in versioned.root.root().find_all(&matcher) {
        let range = convert_node_to_range(&matched_node);
        if !ranges.contains(&range) {
          continue;
        }
        let fixer = match &config.fixer {
          Some(fixer) => fixer,
          None => continue,
        };
        let edit = matched_node.replace_by(fixer);
        let edit = TextEdit {
          range,
          new_text: String::from_utf8(edit.inserted_text).unwrap(),
        };
        let mut changes = HashMap::new();
        changes.insert(text_doc.uri.clone(), vec![edit]);
        let edit = Some(WorkspaceEdit {
          changes: Some(changes),
          document_changes: None,
          change_annotations: None,
        });
        let action = CodeAction {
          title: config.message.clone(),
          command: None,
          diagnostics: None,
          edit,
          disabled: None,
          kind: Some(CodeActionKind::QUICKFIX),
          is_preferred: Some(true),
          data: None,
        };
        response.push(CodeActionOrCommand::from(action));
      }
    }
    Some(response)
  }

  fn build_error_id_to_ranges(diagnostics: Vec<Diagnostic>) -> HashMap<String, Vec<Range>> {
    let mut error_id_to_ranges = HashMap::new();
    for diagnostic in diagnostics {
      let rule_id = match diagnostic.code {
        Some(NumberOrString::String(rule)) => rule,
        _ => continue,
      };
      let ranges = error_id_to_ranges.entry(rule_id).or_insert_with(Vec::new);
      ranges.push(diagnostic.range);
    }
    error_id_to_ranges
  }

  // TODO: support other urls besides file_scheme
  fn infer_lang_from_uri(uri: &Url) -> Option<L> {
    let path = uri.to_file_path().ok()?;
    L::from_path(path)
  }
}
