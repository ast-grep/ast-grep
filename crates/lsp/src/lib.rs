mod utils;

use ast_grep_core::NodeMatch;
use dashmap::DashMap;
use serde_json::Value;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::lsp_types::notification::{DidChangeWatchedFiles, Notification};
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::UriExt;
use tower_lsp_server::{Client, LanguageServer};

use ast_grep_config::{CombinedScan, RuleCollection, Severity};
use ast_grep_core::{
  tree_sitter::{LanguageExt, StrDoc},
  AstGrep, Doc,
};

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use utils::{convert_match_to_diagnostic, diagnostic_to_code_action, Fixes, RewriteData};

pub use tower_lsp_server::{LspService, Server};

pub trait LSPLang: LanguageExt + Eq + Send + Sync + 'static {}
impl<T> LSPLang for T where T: LanguageExt + Eq + Send + Sync + 'static {}

type Notes = BTreeMap<(u32, u32, u32, u32), Arc<String>>;

struct VersionedAst<D: Doc> {
  version: i32,
  root: AstGrep<D>,
  notes: Notes,
  fixes: Fixes,
}

pub struct Backend<L: LSPLang> {
  client: Client,
  map: DashMap<String, VersionedAst<StrDoc<L>>>,
  base: PathBuf,
  rules: Arc<RwLock<RuleCollection<L>>>,
  // interner for rule ids to note, to avoid duplication
  interner: DashMap<String, Arc<String>>,
  // rule finding closure to reload rules
  rule_finder: Box<dyn Fn() -> anyhow::Result<RuleCollection<L>> + Send + Sync>,
  // store client capabilities to check support
  capabilities: Arc<RwLock<ClientCapabilities>>,
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

impl<L: LSPLang> LanguageServer for Backend<L> {
  async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
    let code_action_provider = code_action_provider(&params.capabilities);
    if let Ok(mut cap) = self.capabilities.write() {
      *cap = params.capabilities;
    }
    Ok(InitializeResult {
      server_info: Some(ServerInfo {
        name: "ast-grep language server".to_string(),
        version: None,
      }),
      capabilities: ServerCapabilities {
        // TODO: change this to incremental
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        code_action_provider: code_action_provider.or(FALLBACK_CODE_ACTION_PROVIDER),
        execute_command_provider: Some(ExecuteCommandOptions {
          commands: vec![APPLY_ALL_FIXES.to_string()],
          work_done_progress_options: Default::default(),
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
      },
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    self
      .client
      .log_message(MessageType::INFO, "server initialized!")
      .await;
    if let Err(e) = self.reload_rules().await {
      self
        .client
        .show_message(MessageType::ERROR, format!("Failed to load rules: {e}"))
        .await;
    }

    // Register file watchers for configuration files
    if let Err(e) = self.register_file_watchers().await {
      self
        .client
        .log_message(
          MessageType::ERROR,
          format!("Failed to register file watchers: {e:?}"),
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

  async fn did_change_watched_files(&self, _params: DidChangeWatchedFilesParams) {
    // File watcher already ensures only yml files are watched, so just reload
    self
      .client
      .log_message(
        MessageType::INFO,
        "Configuration files changed, reloading rules...",
      )
      .await;

    if let Err(e) = self.reload_rules().await {
      self
        .client
        .show_message(MessageType::ERROR, format!("Failed to reload rules: {e}"))
        .await;
    } else {
      self
        .client
        .log_message(MessageType::INFO, "Rules reloaded successfully")
        .await;
    }
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

  async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    self
      .client
      .log_message(MessageType::LOG, "Get Hover Notes")
      .await;
    Ok(self.do_hover(params.text_document_position_params))
  }
}

fn pos_tuple_to_range((line, character, end_line, end_character): (u32, u32, u32, u32)) -> Range {
  Range {
    start: Position { line, character },
    end: Position {
      line: end_line,
      character: end_character,
    },
  }
}

impl<L: LSPLang> Backend<L> {
  pub fn new<F>(client: Client, base: PathBuf, rule_finder: F) -> Self
  where
    F: Fn() -> anyhow::Result<RuleCollection<L>> + Send + Sync + 'static,
  {
    Self {
      client,
      rules: Arc::new(RwLock::new(RuleCollection::default())),
      base,
      map: DashMap::new(),
      interner: DashMap::new(),
      rule_finder: Box::new(rule_finder),
      capabilities: Arc::new(RwLock::new(ClientCapabilities::default())),
    }
  }

  /// Convert URI to a path relative to base directory
  fn uri_to_relative_path(&self, uri: &Uri) -> Option<PathBuf> {
    let absolute_path = uri.to_file_path()?;
    if let Ok(relative_path) = absolute_path.strip_prefix(&self.base) {
      Some(relative_path.to_path_buf())
    } else {
      Some(absolute_path.to_path_buf())
    }
  }

  fn do_hover(&self, pos_params: TextDocumentPositionParams) -> Option<Hover> {
    let uri = pos_params.text_document.uri;
    let Position {
      line,
      character: column,
    } = pos_params.position;
    let ast = self.map.get(uri.as_str())?;
    let query = (line, column, line, column);
    // TODO: next_back is not precise, it can return a note that is larger
    let (pos, markdown) = ast.notes.range(..=query).next_back()?;
    // out of range check
    if pos.0 > line || pos.2 < line {
      return None;
    }
    if pos.0 == line && pos.1 > column || pos.2 == line && pos.3 < column {
      return None;
    }
    Some(Hover {
      contents: HoverContents::Markup(MarkupContent {
        kind: MarkupKind::Markdown,
        value: markdown.to_string(),
      }),
      range: Some(pos_tuple_to_range(*pos)),
    })
  }

  fn get_diagnostics(
    &self,
    uri: &Uri,
    versioned: &VersionedAst<StrDoc<L>>,
  ) -> Option<(Vec<Diagnostic>, Fixes)> {
    let path = self.uri_to_relative_path(uri)?;

    let rules = self.rules.read().ok()?;
    let rule_refs = rules.for_path(&path);
    if rule_refs.is_empty() {
      return None;
    }
    let unused_suppression_rule =
      CombinedScan::unused_config(Severity::Hint, rule_refs[0].language.clone());
    let mut scan = CombinedScan::new(rule_refs);
    scan.set_unused_suppression_rule(&unused_suppression_rule);
    let matches = scan.scan(&versioned.root, false).matches;
    let mut diagnostics = vec![];
    let mut fixes = Fixes::new();
    for (rule, ms) in matches {
      let to_diagnostic = |m: NodeMatch<StrDoc<L>>| {
        let diagnostic = convert_match_to_diagnostic(uri, &m, rule);
        let rewrite_data = RewriteData::from_node_match(&m, rule);
        if let Some(r) = rewrite_data {
          fixes.insert((diagnostic.range, rule.id.clone()), r);
        }
        diagnostic
      };
      diagnostics.extend(ms.into_iter().map(to_diagnostic));
    }
    Some((diagnostics, fixes))
  }

  fn build_notes(&self, diagnostics: &[Diagnostic]) -> Notes {
    let mut notes = BTreeMap::new();
    for diagnostic in diagnostics {
      let Some(NumberOrString::String(id)) = &diagnostic.code else {
        continue;
      };
      let Ok(rules) = self.rules.read() else {
        continue;
      };
      let Some(note) = rules.get_rule(id).and_then(|r| r.note.clone()) else {
        continue;
      };
      let start = diagnostic.range.start;
      let end = diagnostic.range.end;
      let atom = self
        .interner
        .entry(id.clone())
        .or_insert_with(|| Arc::new(note.clone()))
        .clone();
      notes.insert((start.line, start.character, end.line, end.character), atom);
    }
    notes
  }

  async fn publish_diagnostics(
    &self,
    uri: Uri,
    versioned: &mut VersionedAst<StrDoc<L>>,
  ) -> Option<()> {
    let (diagnostics, fixes) = self.get_diagnostics(&uri, versioned).unwrap_or_default();
    versioned.notes = self.build_notes(&diagnostics);
    versioned.fixes = fixes;

    self
      .client
      .publish_diagnostics(uri, diagnostics, Some(versioned.version))
      .await;
    Some(())
  }

  async fn get_path_of_first_workspace(&self) -> Option<std::path::PathBuf> {
    // need drop the lock before await
    let client_support_workspace = {
      let cap = self.capabilities.read().ok()?;
      cap
        .workspace
        .as_ref()
        .and_then(|w| w.workspace_folders)
        .unwrap_or(false)
    };
    if !client_support_workspace {
      return None;
    }
    let folders = self.client.workspace_folders().await.ok()??;
    let folder = folders.first()?;
    folder.uri.to_file_path().map(PathBuf::from)
  }

  // skip files outside of workspace root #1382, #1402
  async fn should_skip_file_outside_workspace(&self, text_doc: &TextDocumentItem) -> Option<()> {
    // fallback to base if no workspace provided by client #2211
    let workspace_root = self
      .get_path_of_first_workspace()
      .await
      .unwrap_or_else(|| self.base.clone());
    let doc_file_path = text_doc.uri.to_file_path()?;
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
    let mut versioned = VersionedAst {
      version: text_doc.version,
      root,
      notes: BTreeMap::new(),
      fixes: Fixes::new(),
    };
    self
      .client
      .log_message(MessageType::LOG, "Publishing init diagnostics.")
      .await;
    self.publish_diagnostics(text_doc.uri, &mut versioned).await;
    self.map.insert(uri.to_owned(), versioned); // don't lock dashmap
    Some(())
  }

  async fn on_change(&self, params: DidChangeTextDocumentParams) -> Option<()> {
    let text_doc = params.text_document;
    let uri = text_doc.uri.as_str();
    let change = &params.content_changes.first()?;
    let text = &change.text;
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
      notes: BTreeMap::new(),
      fixes: Fixes::new(),
    };
    self
      .client
      .log_message(MessageType::LOG, "Publishing diagnostics.")
      .await;
    self
      .publish_diagnostics(text_doc.uri, &mut *versioned)
      .await;
    Some(())
  }
  async fn on_close(&self, params: DidCloseTextDocumentParams) {
    self.map.remove(params.text_document.uri.as_str());
  }

  fn compute_all_fixes(
    &self,
    text_document: TextDocumentIdentifier,
  ) -> std::result::Result<HashMap<Uri, Vec<TextEdit>>, LspError>
  where
    L: ast_grep_core::Language + std::cmp::Eq,
  {
    let uri = text_document.uri;
    let versioned = self
      .map
      .get(uri.as_str())
      .ok_or(LspError::UnsupportedFileType)?;
    let (_diagnostics, fixes) = self
      .get_diagnostics(&uri, &versioned)
      .ok_or(LspError::NoActionableFix)?;

    let mut entries: Vec<_> = fixes.iter().collect();
    entries.sort_by(|((range_a, _), _), ((range_b, _), _)| {
      range_a
        .start
        .cmp(&range_b.start)
        .then(range_a.end.cmp(&range_b.end))
    });

    let mut last = Position {
      line: 0,
      character: 0,
    };
    // TODO: the vec may not correct since sorting is based on diagnostic range
    // instead of fix range, which can be different in the case of expanded_start/end
    let edits: Vec<TextEdit> = entries
      .into_iter()
      .filter_map(|((range, _id), rewrite_data)| {
        if range.start < last {
          return None;
        }
        let first_fix = rewrite_data.fixers.first()?;
        let fixed = first_fix.fixed.to_string();
        // compute the expanded fix range
        let range = first_fix.range.as_ref().unwrap_or(range);
        let edit = TextEdit::new(*range, fixed);
        last = range.end;
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

    let document = self.map.get(text_doc.uri.as_str())?;
    let fixes_cache = &document.fixes;

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
      .filter_map(|d| diagnostic_to_code_action(&text_doc, d, fixes_cache))
      .flatten()
      .map(CodeActionOrCommand::from)
      .collect();
    Some(response)
  }

  // TODO: support other urls besides file_scheme
  fn infer_lang_from_uri(uri: &Uri) -> Option<L> {
    let path = uri.to_file_path()?;
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
          .log_message(MessageType::LOG, format!("Unrecognized command: {command}"))
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
        format!("Running ExecuteCommand {command}"),
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
            format!("JSON deserialization error: {e}"),
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

  /// Register file watchers for configuration files
  async fn register_file_watchers(
    &self,
  ) -> std::result::Result<(), tower_lsp_server::jsonrpc::Error> {
    let yml_watcher = FileSystemWatcher {
      glob_pattern: GlobPattern::String("**/*.{yml,yaml}".to_string()),
      kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
    };
    let registration = Registration {
      id: "ast-grep-config-watcher".to_string(),
      method: DidChangeWatchedFiles::METHOD.to_string(),
      register_options: Some(
        serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
          watchers: vec![yml_watcher],
        })
        .map_err(|e| tower_lsp_server::jsonrpc::Error::invalid_params(e.to_string()))?,
      ),
    };

    self.client.register_capability(vec![registration]).await
  }

  /// Reload rules from configuration and republish diagnostics for all open files
  async fn reload_rules(&self) -> anyhow::Result<()> {
    self
      .client
      .log_message(MessageType::INFO, "Starting rule reload...")
      .await;

    match (self.rule_finder)() {
      Ok(new_rules) => {
        // Update the rules
        {
          let mut rules = self
            .rules
            .write()
            .map_err(|e| anyhow::anyhow!("Lock error: {e}"))?;
          *rules = new_rules;
        }

        self
          .client
          .log_message(
            MessageType::INFO,
            "Rules reloaded successfully using CLI logic",
          )
          .await;
      }
      Err(error) => {
        // popup message
        self
          .client
          .show_message(MessageType::ERROR, format!("Failed to load rules: {error}"))
          .await;
        // log message
        self
          .client
          .log_message(MessageType::ERROR, format!("Failed to load rules: {error}"))
          .await;
      }
    }

    // Clear the interner since rule IDs might have changed
    self.interner.clear();

    // Republish diagnostics for all open files
    self.republish_all_diagnostics().await;

    Ok(())
  }

  /// Republish diagnostics for all currently open files
  async fn republish_all_diagnostics(&self) {
    // Get all currently open file URIs
    for mut entry in self.map.iter_mut() {
      let (uri_str, versioned) = entry.pair_mut();
      let Ok(uri) = uri_str.parse::<Uri>() else {
        continue;
      };
      // Republish diagnostics for this file
      let (diagnostics, fixes) = match self.get_diagnostics(&uri, versioned) {
        Some((d, f)) => (d, f),
        None => (Vec::new(), HashMap::new()),
      };
      versioned.notes = self.build_notes(&diagnostics);
      versioned.fixes = fixes;
      self
        .client
        .publish_diagnostics(uri, diagnostics, Some(versioned.version))
        .await;
    }
  }
}

enum LspError {
  JSONDecodeError(serde_json::Error),
  UnsupportedFileType,
  NoActionableFix,
}
