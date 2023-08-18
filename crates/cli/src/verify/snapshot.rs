use crate::lang::SgLang;
use ast_grep_core::{NodeMatch, StrDoc};

use super::Node;
use serde::{Deserialize, Serialize, Serializer};

use std::collections::{BTreeMap, HashMap};

type CaseId = String;
type Source = String;

// TODO: add comment
pub type SnapshotCollection = HashMap<CaseId, TestSnapshots>;

// TODO: add comment
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestSnapshots {
  pub id: CaseId,
  #[serde(serialize_with = "ordered_map")]
  pub snapshots: HashMap<Source, TestSnapshot>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestSnapshot {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fixed: Option<String>,
  pub labels: Vec<Label>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum LabelStyle {
  Primary,
  Secondary,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Label {
  // TODO: change visibility
  pub source: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  message: Option<String>,
  style: LabelStyle,
  start: usize,
  end: usize,
}

impl Label {
  fn primary(n: &Node<SgLang>) -> Self {
    let range = n.range();
    Self {
      source: n.text().to_string(),
      message: None,
      style: LabelStyle::Primary,
      start: range.start,
      end: range.end,
    }
  }

  fn secondary(n: &Node<SgLang>) -> Self {
    let range = n.range();
    Self {
      source: n.text().to_string(),
      message: None,
      style: LabelStyle::Secondary,
      start: range.start,
      end: range.end,
    }
  }

  // TODO: change visibility
  pub fn from_matched(n: NodeMatch<StrDoc<SgLang>>) -> Vec<Self> {
    let mut ret = vec![Self::primary(&n)];
    if let Some(secondary) = n.get_env().get_labels("secondary") {
      ret.extend(secondary.iter().map(Self::secondary));
    }
    ret
  }
}

fn ordered_map<S>(value: &HashMap<String, TestSnapshot>, serializer: S) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  let ordered: BTreeMap<_, _> = value.iter().collect();
  ordered.serialize(serializer)
}
