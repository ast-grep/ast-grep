mod utils;

use dashmap::DashMap;
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use ast_grep_config::{CombinedScan, RuleCollection, RuleConfig, Severity};
use ast_grep_core::{language::Language, AstGrep, Doc, StrDoc};

use std::collections::HashMap;
use std::path::PathBuf;

use utils::{convert_match_to_diagnostic, diagnostic_to_code_action, RewriteData};

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
  base: PathBuf,
  rules: std::result::Result<RuleCollection<L>, String>,
}

const FALLBACK_CODE_ACTION_PROVIDER: Option<CodeActionProviderCapability> =
  Some(CodeActionProviderCapability::Simple(true));

const APPLY_ALL_FIXES: &str = "ast-grep.applyAllFixes";
const QUICKFIX_AST_GREP: &str = "quickfix.ast-grep";
const FIX_ALL_AST_GREP: &str = "source.fixAll.ast-grep";

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
    code_action_kinds: Some(vec![
      CodeActionKind::new(QUICKFIX_AST_GREP),
      CodeActionKind::new(FIX_ALL_AST_GREP),
    ]),
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
          .or(FALLBACK_CODE_ACTION_PROVIDER),
        execute_command_provider: Some(ExecuteCommandOptions {
          commands: vec![APPLY_ALL_FIXES.to_string()],
          work_done_progress_options: Default::default(),
        }),
        ..ServerCapabilities::default()
      },
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    self
      .client
      .log_message(MessageType::INFO, "server initialized!")
      .await;

    // Report errors loading config once, upon initialization
    if let Err(error) = &self.rules {
      // popup message
      self
        .client
        .show_message(
          MessageType::ERROR,
          format!("Failed to load rules: {}", error),
        )
        .await;
      // log message
      self
        .client
        .log_message(
          MessageType::ERROR,
          format!("Failed to load rules: {}", error),
        )
        .await;
    }
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
    Ok(self.on_code_action(params).await)
  }

  async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
    Ok(self.on_execute_command(params).await)
  }
}

impl<L: LSPLang> Backend<L> {
  pub fn new(
    client: Client,
    base: PathBuf,
    rules: std::result::Result<RuleCollection<L>, String>,
  ) -> Self {
    Self {
      client,
      rules,
      base,
      map: DashMap::new(),
    }
  }

  fn get_rules(&self, uri: &Url) -> Option<Vec<&RuleConfig<L>>> {
    let absolute_path = uri.to_file_path().ok()?;
    let path = if let Ok(p) = absolute_path.strip_prefix(&self.base) {
      p
    } else {
      &absolute_path
    };
    let rules = self.rules.as_ref().ok()?.for_path(path);
    Some(rules)
  }

  fn get_diagnostics(
    &self,
    uri: &Url,
    versioned: &VersionedAst<StrDoc<L>>,
  ) -> Option<Vec<Diagnostic>> {
    let rules = self.get_rules(uri)?;
    if rules.is_empty() {
      return None;
    }
    let unused_suppression_rule =
      CombinedScan::unused_config(Severity::Hint, rules[0].language.clone());
    let mut scan = CombinedScan::new(rules);
    scan.set_unused_suppression_rule(&unused_suppression_rule);
    let matches = scan.scan(&versioned.root, false).matches;
    let mut diagnostics = vec![];
    for (rule, ms) in matches {
      let to_diagnostic = |m| convert_match_to_diagnostic(m, rule);
      diagnostics.extend(ms.into_iter().map(to_diagnostic));
    }
    Some(diagnostics)
  }

  async fn publish_diagnostics(&self, uri: Url, versioned: &VersionedAst<StrDoc<L>>) -> Option<()> {
    let diagnostics = self.get_diagnostics(&uri, versioned).unwrap_or_default();
    self
      .client
      .publish_diagnostics(uri, diagnostics, Some(versioned.version))
      .await;
    Some(())
  }

  async fn get_path_of_first_workspace(&self) -> Option<std::path::PathBuf> {
    let folders = self.client.workspace_folders().await.ok()??;
    let folder = folders.first()?;
    folder.uri.to_file_path().ok()
  }

  // skip files outside of workspace root #1382, #1402
  async fn should_skip_file_outside_workspace(&self, text_doc: &TextDocumentItem) -> Option<()> {
    let workspace_root = self.get_path_of_first_workspace().await?;
    let doc_file_path = text_doc.uri.to_file_path().ok()?;
    if doc_file_path.starts_with(workspace_root) {
      None
    } else {
      Some(())
    }
  }

  async fn on_open(&self, params: DidOpenTextDocumentParams) -> Option<()> {
    let text_doc = params.text_document;
    if self
      .should_skip_file_outside_workspace(&text_doc)
      .await
      .is_some()
    {
      return None;
    }
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

  fn compute_all_fixes(
    &self,
    text_document: TextDocumentIdentifier,
  ) -> std::result::Result<HashMap<Url, Vec<TextEdit>>, LspError>
  where
    L: ast_grep_core::Language + std::cmp::Eq,
  {
    let uri = text_document.uri;
    let versioned = self
      .map
      .get(uri.as_str())
      .ok_or(LspError::UnsupportedFileType)?;
    let mut diagnostics = self
      .get_diagnostics(&uri, &versioned)
      .ok_or(LspError::NoActionableFix)?;
    diagnostics.sort_by_key(|d| (d.range.start, d.range.end));
    let mut last = Position {
      line: 0,
      character: 0,
    };
    let edits: Vec<_> = diagnostics
      .into_iter()
      .filter_map(|d| {
        if d.range.start < last {
          return None;
        }
        let rewrite_data = RewriteData::from_value(d.data?)?;
        let edit = TextEdit::new(d.range, rewrite_data.fixed);
        last = d.range.end;
        Some(edit)
      })
      .collect();
    if edits.is_empty() {
      return Err(LspError::NoActionableFix);
    }
    let mut changes = HashMap::new();
    changes.insert(uri, edits);
    Ok(changes)
  }

  async fn on_code_action(&self, params: CodeActionParams) -> Option<CodeActionResponse> {
    if let Some(kinds) = params.context.only.as_ref() {
      if kinds.contains(&CodeActionKind::SOURCE_FIX_ALL) {
        return self.fix_all_code_action(params.text_document);
      }
    }
    self.quickfix_code_action(params)
  }

  fn fix_all_code_action(
    &self,
    text_document: TextDocumentIdentifier,
  ) -> Option<CodeActionResponse> {
    let fixed = self.compute_all_fixes(text_document).ok()?;
    let edit = WorkspaceEdit::new(fixed);
    let code_action = CodeAction {
      title: "Fix by ast-grep".into(),
      command: None,
      diagnostics: None,
      edit: Some(edit),
      kind: Some(CodeActionKind::new(FIX_ALL_AST_GREP)),
      is_preferred: None,
      data: None,
      disabled: None,
    };
    Some(vec![CodeActionOrCommand::CodeAction(code_action)])
  }

  fn quickfix_code_action(&self, params: CodeActionParams) -> Option<CodeActionResponse> {
    if params.context.diagnostics.is_empty() {
      return None;
    }
    let text_doc = params.text_document;
    let response = params
      .context
      .diagnostics
      .into_iter()
      .filter(|d| {
        d.source
          .as_ref()
          .map(|s| s.contains("ast-grep"))
          .unwrap_or(false)
      })
      .filter_map(|d| diagnostic_to_code_action(&text_doc, d))
      .map(CodeActionOrCommand::from)
      .collect();
    Some(response)
  }

  // TODO: support other urls besides file_scheme
  fn infer_lang_from_uri(uri: &Url) -> Option<L> {
    let path = uri.to_file_path().ok()?;
    L::from_path(path)
  }

  async fn on_execute_command(&self, params: ExecuteCommandParams) -> Option<Value> {
    let ExecuteCommandParams {
      arguments,
      command,
      work_done_progress_params: _,
    } = params;

    match command.as_ref() {
      APPLY_ALL_FIXES => {
        self.on_apply_all_fix(command, arguments).await?;
        None
      }
      _ => {
        self
          .client
          .log_message(
            MessageType::LOG,
            format!("Unrecognized command: {}", command),
          )
          .await;
        None
      }
    }
  }

  async fn on_apply_all_fix_impl(
    &self,
    first: Value,
  ) -> std::result::Result<WorkspaceEdit, LspError> {
    let text_doc: TextDocumentItem =
      serde_json::from_value(first).map_err(LspError::JSONDecodeError)?;
    let uri = text_doc.uri;
    // let version = text_doc.version;
    let changes = self.compute_all_fixes(TextDocumentIdentifier::new(uri))?;
    let workspace_edit = WorkspaceEdit {
      changes: Some(changes),
      document_changes: None,
      change_annotations: None,
    };
    Ok(workspace_edit)
  }

  async fn on_apply_all_fix(&self, command: String, arguments: Vec<Value>) -> Option<()> {
    self
      .client
      .log_message(
        MessageType::INFO,
        format!("Running ExecuteCommand {}", command),
      )
      .await;
    let first = arguments.first()?.clone();
    let workspace_edit = match self.on_apply_all_fix_impl(first).await {
      Ok(workspace_edit) => workspace_edit,
      Err(error) => {
        self.report_error(error).await;
        return None;
      }
    };
    self.client.apply_edit(workspace_edit).await.ok()?;
    None
  }

  async fn report_error(&self, error: LspError) {
    match error {
      LspError::JSONDecodeError(e) => {
        self
          .client
          .log_message(
            MessageType::ERROR,
            format!("JSON deserialization error: {}", e),
          )
          .await;
      }
      LspError::UnsupportedFileType => {
        self
          .client
          .log_message(MessageType::ERROR, "Unsupported file type")
          .await;
      }
      LspError::NoActionableFix => {
        self
          .client
          .log_message(MessageType::LOG, "No actionable fix")
          .await;
      }
    }
  }
}

enum LspError {
  JSONDecodeError(serde_json::Error),
  UnsupportedFileType,
  NoActionableFix,
}
