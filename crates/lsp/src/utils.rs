//! Provides utility to convert ast-grep data types to lsp data types
use ast_grep_config::RuleConfig;
use ast_grep_config::Severity;
use ast_grep_core::{language::Language, Doc, Node, NodeMatch, StrDoc};

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::*;

use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct RewriteData {
  pub fixed: String,
  // maybe we should have fixed range
}

impl RewriteData {
  pub fn from_value(data: serde_json::Value) -> Option<Self> {
    serde_json::from_value(data).ok()
  }

  fn from_node_match<L: Language>(
    node_match: &NodeMatch<StrDoc<L>>,
    rule: &RuleConfig<L>,
  ) -> Option<Self> {
    let fixer = rule.matcher.fixer.as_ref()?;
    let edit = node_match.replace_by(fixer);
    let rewrite = String::from_utf8(edit.inserted_text).ok()?;
    Some(Self { fixed: rewrite })
  }
}

pub fn diagnostic_to_code_action(
  text_doc: &TextDocumentIdentifier,
  diagnostic: Diagnostic,
) -> Option<CodeAction> {
  let rewrite_data = RewriteData::from_value(diagnostic.data?)?;
  let mut changes = HashMap::new();
  let text_edit = TextEdit::new(diagnostic.range, rewrite_data.fixed);
  changes.insert(text_doc.uri.clone(), vec![text_edit]);

  let edit = WorkspaceEdit::new(changes);
  let NumberOrString::String(id) = diagnostic.code? else {
    return None;
  };
  let action = CodeAction {
    title: format!("Fix `{id}` with ast-grep"),
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

fn convert_node_to_range<D: Doc>(node_match: &Node<D>) -> Range {
  let start = node_match.start_pos();
  let end = node_match.end_pos();
  Range {
    start: Position {
      line: start.line() as u32,
      character: start.column(node_match) as u32,
    },
    end: Position {
      line: end.line() as u32,
      character: end.column(node_match) as u32,
    },
  }
}

pub fn convert_match_to_diagnostic<L: Language>(
  node_match: NodeMatch<StrDoc<L>>,
  rule: &RuleConfig<L>,
) -> Diagnostic {
  // TODO
  let rewrite_data =
    RewriteData::from_node_match(&node_match, rule).and_then(|r| serde_json::to_value(r).ok());
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
    related_information: None,
    data: rewrite_data,
  }
}

fn get_non_empty_message<L: Language>(rule: &RuleConfig<L>, nm: &NodeMatch<StrDoc<L>>) -> String {
  // Note: The LSP client in vscode won't show any diagnostics at all if it receives one with an empty message
  let msg = if rule.message.is_empty() {
    rule.id.to_string()
  } else {
    rule.get_message(nm)
  };
  // append note to message ast-grep/ast-grep-vscode#352
  if let Some(note) = &rule.note {
    format!("{msg}\n\n{note}")
  } else {
    msg
  }
}

fn url_to_code_description(url: &Option<String>) -> Option<CodeDescription> {
  let href = Url::parse(url.as_ref()?).ok()?;
  Some(CodeDescription { href })
}
