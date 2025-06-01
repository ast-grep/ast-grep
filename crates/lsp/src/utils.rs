//! Provides utility to convert ast-grep data types to lsp data types
use ast_grep_config::Label;
use ast_grep_config::LabelStyle;
use ast_grep_config::RuleConfig;
use ast_grep_config::Severity;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc};
use ast_grep_core::{Doc, Node, NodeMatch};

use serde::{Deserialize, Serialize};
use tower_lsp_server::lsp_types::*;

use std::collections::HashMap;
use std::str::FromStr;

#[derive(Serialize, Deserialize)]
pub struct OneFix {
  pub title: Option<String>,
  pub fixed: String,
}

#[derive(Serialize, Deserialize)]
pub struct RewriteData {
  pub fixers: Vec<OneFix>,
  // maybe we should have fixed range
}

impl RewriteData {
  pub fn from_value(data: serde_json::Value) -> Option<Self> {
    serde_json::from_value(data).ok()
  }

  fn from_node_match<L: LanguageExt>(
    node_match: &NodeMatch<StrDoc<L>>,
    rule: &RuleConfig<L>,
  ) -> Option<Self> {
    let fixers = rule
      .matcher
      .fixer
      .iter()
      .filter_map(|fixer| {
        let edit = node_match.make_edit(&rule.matcher, fixer);
        let range = node_match.range();
        // skip if the edit does not match the range
        // TODO: implement expanding range
        if edit.position != range.start || edit.deleted_length != range.len() {
          return None;
        }
        let rewrite = String::from_utf8(edit.inserted_text).ok()?;
        Some(OneFix {
          title: fixer.title().map(ToString::to_string),
          fixed: rewrite,
        })
      })
      .collect();
    Some(Self { fixers })
  }
}

pub fn diagnostic_to_code_action(
  text_doc: &TextDocumentIdentifier,
  diagnostic: Diagnostic,
) -> Option<Vec<CodeAction>> {
  let rewrite_data = RewriteData::from_value(diagnostic.data?)?;
  let actions = rewrite_data.fixers.into_iter().filter_map(|fixer| {
    let mut changes = HashMap::new();
    let text_edit = TextEdit::new(diagnostic.range, fixer.fixed);
    changes.insert(text_doc.uri.clone(), vec![text_edit]);

    let edit = WorkspaceEdit::new(changes);
    let NumberOrString::String(id) = diagnostic.code.as_ref()? else {
      return None;
    };
    let title = fixer
      .title
      .unwrap_or_else(|| format!("Fix `{}` with ast-grep", id));
    Some(CodeAction {
      title,
      command: None,
      diagnostics: None,
      edit: Some(edit),
      disabled: None,
      kind: Some(CodeActionKind::QUICKFIX),
      is_preferred: Some(true),
      data: None,
    })
  });
  Some(actions.collect())
}

fn convert_nodes_to_range<D: Doc>(start_node: &Node<D>, end_node: &Node<D>) -> Range {
  let start = start_node.start_pos();
  let end = end_node.end_pos();
  Range {
    start: Position {
      line: start.line() as u32,
      character: start.column(start_node) as u32,
    },
    end: Position {
      line: end.line() as u32,
      character: end.column(end_node) as u32,
    },
  }
}

fn get_related_info<L: LanguageExt>(
  uri: &Uri,
  labels: &[Label<StrDoc<L>>],
) -> Option<Vec<DiagnosticRelatedInformation>> {
  labels
    .iter()
    .filter_map(|label| {
      let message = label.message?;
      let range = convert_nodes_to_range(&label.start_node, &label.end_node);
      Some(DiagnosticRelatedInformation {
        location: Location {
          uri: uri.clone(),
          range,
        },
        message: message.to_string(),
      })
    })
    .collect::<Vec<_>>()
    .into()
}

fn get_primary_label<L: LanguageExt>(
  node_match: &NodeMatch<StrDoc<L>>,
  labels: &[Label<StrDoc<L>>],
) -> Range {
  let Some(label) = labels.iter().find(|l| l.style == LabelStyle::Primary) else {
    return convert_nodes_to_range(node_match, node_match);
  };
  let start = label.start_node.start_pos();
  let end = label.end_node.end_pos();
  Range {
    start: Position {
      line: start.line() as u32,
      character: start.column(&label.start_node) as u32,
    },
    end: Position {
      line: end.line() as u32,
      character: end.column(&label.end_node) as u32,
    },
  }
}

fn get_node_range_and_related_info<L: LanguageExt>(
  uri: &Uri,
  node_match: &NodeMatch<StrDoc<L>>,
  rule: &RuleConfig<L>,
) -> (Range, Option<Vec<DiagnosticRelatedInformation>>) {
  // if user has not specified any labels, we don't need to show anything
  // the default labels are pretty noisy
  if rule.labels.is_none() {
    let range = convert_nodes_to_range(node_match, node_match);
    return (range, None);
  }
  let labels = rule.get_labels(node_match);
  let related_information = get_related_info(uri, &labels);
  (get_primary_label(node_match, &labels), related_information)
}

pub fn convert_match_to_diagnostic<L: LanguageExt>(
  uri: &Uri,
  node_match: NodeMatch<StrDoc<L>>,
  rule: &RuleConfig<L>,
) -> Diagnostic {
  let rewrite_data =
    RewriteData::from_node_match(&node_match, rule).and_then(|r| serde_json::to_value(r).ok());
  let (range, related_information) = get_node_range_and_related_info(uri, &node_match, rule);
  Diagnostic {
    range,
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
    related_information,
    data: rewrite_data,
  }
}

fn get_non_empty_message<L: LanguageExt>(
  rule: &RuleConfig<L>,
  nm: &NodeMatch<StrDoc<L>>,
) -> String {
  // Note: The LSP client in vscode won't show any diagnostics at all if it receives one with an empty message
  if rule.message.is_empty() {
    rule.id.to_string()
  } else {
    rule.get_message(nm)
  }
}

fn url_to_code_description(url: &Option<String>) -> Option<CodeDescription> {
  let href = Uri::from_str(url.as_ref()?).ok()?;
  Some(CodeDescription { href })
}
