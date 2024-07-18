use ast_grep_config::RuleConfig;
use ast_grep_config::Severity;
use ast_grep_core::{language::Language, Doc, Node, NodeMatch, StrDoc};

use tower_lsp::lsp_types::*;

use std::collections::HashMap;

pub fn diagnostic_to_code_action(
  text_doc: &TextDocumentIdentifier,
  diagnostic: Diagnostic,
) -> Option<CodeAction> {
  let data = diagnostic.data?;
  // TODO
  let map: HashMap<String, String> = serde_json::from_value(data).ok()?;
  let rewrite = map.get("fixed")?.to_string();
  let mut changes = HashMap::new();
  let text_edit = TextEdit::new(diagnostic.range, rewrite);
  changes.insert(text_doc.uri.clone(), vec![text_edit]);
  let edit = WorkspaceEdit::new(changes);
  let action = CodeAction {
    // TODO
    title: format!("Fix `{:?}` with ast-grep", diagnostic.code),
    command: None,
    diagnostics: None,
    edit: Some(edit),
    disabled: None,
    kind: Some(CodeActionKind::QUICKFIX),
    is_preferred: Some(true),
    data: None,
  };
  Some(action)
}

pub fn convert_node_to_range<D: Doc>(node_match: &Node<D>) -> Range {
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

pub fn convert_match_to_diagnostic<L: Language>(
  node_match: NodeMatch<StrDoc<L>>,
  rule: &RuleConfig<L>,
  uri: &Url,
) -> Diagnostic {
  // TODO
  let rewrite_data = rule.matcher.fixer.as_ref().and_then(|fixer| {
    let edit = node_match.replace_by(fixer);
    let rewrite = String::from_utf8(edit.inserted_text).ok()?;
    let mut map = HashMap::new();
    map.insert("fixed", rewrite);
    serde_json::to_value(map).ok()
  });
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
    message: get_non_empty_message(rule, &node_match),
    source: Some(String::from("ast-grep")),
    tags: None,
    related_information: collect_labels(&node_match, uri),
    data: rewrite_data,
  }
}

fn get_non_empty_message<L: Language>(rule: &RuleConfig<L>, nm: &NodeMatch<StrDoc<L>>) -> String {
  // Note: The LSP client in vscode won't show any diagnostics at all if it receives one with an empty message
  if rule.message.is_empty() {
    rule.id.to_string()
  } else {
    rule.get_message(nm)
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
