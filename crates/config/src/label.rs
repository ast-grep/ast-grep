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

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct Label<'a> {
  pub style: LabelStyle,
  pub range: Range<usize>,
  pub message: Option<&'a str>,
}

impl Label<'_> {
  fn primary(n: &Node<impl Doc>) -> Self {
    Self {
      style: LabelStyle::Primary,
      range: n.range(),
      message: None,
    }
  }
  fn secondary(n: &Node<impl Doc>) -> Self {
    Self {
      style: LabelStyle::Secondary,
      range: n.range(),
      message: None,
    }
  }
}

pub fn get_labels_from_config<'a>(
  config: &'a HashMap<String, LabelConfig>,
  node_match: &NodeMatch<impl Doc>,
) -> Vec<Label<'a>> {
  let env = node_match.get_env();
  config
    .iter()
    .filter_map(|(var, conf)| {
      let range = if let Some(n) = env.get_match(var) {
        n.range()
      } else {
        let ns = env.get_multiple_matches(var);
        let start = ns.first()?.range().start;
        let end = ns.last()?.range().end;
        start..end
      };
      Some(Label {
        style: conf.style.clone(),
        range,
        message: conf.message.as_deref(),
      })
    })
    .collect()
}

pub fn get_default_labels(n: &NodeMatch<impl Doc>) -> Vec<Label<'static>> {
  let mut ret = vec![Label::primary(n)];
  if let Some(secondary) = n.get_env().get_labels("secondary") {
    ret.extend(secondary.iter().map(Label::secondary));
  }
  ret
}
