use ast_grep_core::{Doc, Node, NodeMatch};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Range};

#[derive(Serialize, Deserialize, Clone, JsonSchema, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum LabelStyle {
  /// Labels that describe the primary cause of a diagnostic.
  Primary,
  /// Labels that provide additional context for a diagnostic.
  Secondary,
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct LabelConfig {
  pub style: LabelStyle,
  pub message: Option<String>,
}

/// A label is a way to mark a specific part of the code with a styled message.
/// It is used to provide diagnostic information in LSP or CLI.
/// 'r represents a lifetime for the message string from `rule`.
/// 't represents a lifetime for the node from a ast `tree`.
pub struct Label<'r, 't, D: Doc> {
  pub style: LabelStyle,
  pub message: Option<&'r str>,
  pub start_node: Node<'t, D>,
  pub end_node: Node<'t, D>,
}

impl<'t, D: Doc> Label<'_, 't, D> {
  fn primary(n: &Node<'t, D>) -> Self {
    Self {
      style: LabelStyle::Primary,
      start_node: n.clone(),
      end_node: n.clone(),
      message: None,
    }
  }
  fn secondary(n: &Node<'t, D>) -> Self {
    Self {
      style: LabelStyle::Secondary,
      start_node: n.clone(),
      end_node: n.clone(),
      message: None,
    }
  }

  pub fn range(&self) -> Range<usize> {
    let start = self.start_node.range().start;
    let end = self.end_node.range().end;
    start..end
  }
}

pub fn get_labels_from_config<'r, 't, D: Doc>(
  config: &'r HashMap<String, LabelConfig>,
  node_match: &NodeMatch<'t, D>,
) -> Vec<Label<'r, 't, D>> {
  let env = node_match.get_env();
  config
    .iter()
    .filter_map(|(var, conf)| {
      let (start, end) = if let Some(n) = env.get_match(var) {
        (n.clone(), n.clone())
      } else {
        let ns = env.get_multiple_matches(var);
        let start = ns.first()?.clone();
        let end = ns.last()?.clone();
        (start, end)
      };
      Some(Label {
        style: conf.style.clone(),
        message: conf.message.as_deref(),
        start_node: start,
        end_node: end,
      })
    })
    .collect()
}

pub fn get_default_labels<'t, D: Doc>(n: &NodeMatch<'t, D>) -> Vec<Label<'static, 't, D>> {
  let mut ret = vec![Label::primary(n)];
  if let Some(secondary) = n.get_env().get_labels("secondary") {
    ret.extend(secondary.iter().map(Label::secondary));
  }
  ret
}
