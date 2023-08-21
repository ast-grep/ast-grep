use crate::lang::SgLang;
use anyhow::Result;
use ast_grep_config::RuleConfig;
use ast_grep_core::{Language, NodeMatch, StrDoc};

use super::{CaseResult, Node};
use serde::{Deserialize, Serialize, Serializer};

use std::collections::{BTreeMap, HashMap};

type CaseId = String;
type Source = String;

/// A collection of test snapshots for different rules
/// where each [TestSnapshots] is identified by its rule ID.
pub type SnapshotCollection = HashMap<CaseId, TestSnapshots>;

fn merge_snapshots(
  accepted: SnapshotCollection,
  mut existing: SnapshotCollection,
) -> SnapshotCollection {
  for (id, tests) in accepted {
    if let Some(existing) = existing.get_mut(&id) {
      existing.snapshots.extend(tests.snapshots);
    } else {
      existing.insert(id, tests);
    }
  }
  existing
}

/// Represents user's decision when [CaseStatus::Wrong].
/// Snapshot update can be accepted or rejected.
#[derive(Debug)]
pub enum SnapshotAction {
  /// Accept all changes
  AcceptAll,
  /// Reject all changes.
  AcceptNone,
  /// Delete outdated snapshots.
  Selectively(SnapshotCollection),
}

impl SnapshotAction {
  pub fn update_snapshot_collection(
    self,
    existing: SnapshotCollection,
    results: &[CaseResult],
  ) -> Option<SnapshotCollection> {
    let accepted = match self {
      Self::AcceptAll => {
        let mut snapshot_collection = SnapshotCollection::new();
        for result in results {
          let case_id = result.id.to_string();
          snapshot_collection.insert(case_id.clone(), result.changed_snapshots());
        }
        snapshot_collection
      }
      Self::AcceptNone => return None,
      Self::Selectively(a) => a,
    };
    Some(merge_snapshots(accepted, existing))
  }
}

/// A list of test snapshots for one specific rule-test identified by its `CaseId`.
/// A test yaml for one rule have multiple valid/invalid test cases.
/// Each invalid code test case has its [TestSnapshot].
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

impl TestSnapshot {
  /// Generate snapshot from rule and test case code
  // Ideally we should return Option<Result<T>>
  // because Some/None indicates if we have found matches,
  // then Result<T> indicates if we have error during replace
  // But to reuse anyhow we use the Result<Option<T>>
  pub fn generate(rule_config: &RuleConfig<SgLang>, case: &str) -> Result<Option<Self>> {
    let mut sg = rule_config.language.ast_grep(case);
    let rule = &rule_config.matcher;
    let Some(matched) = sg.root().find(rule) else {
      return Ok(None);
    };
    let labels = Label::from_matched(matched);
    let Some(fix) = &rule_config.fixer else {
      return Ok(Some(Self { fixed: None, labels }));
    };
    let changed = sg.replace(rule, fix)?;
    debug_assert!(changed);
    Ok(Some(Self {
      fixed: Some(sg.source().to_string()),
      labels,
    }))
  }
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

  fn from_matched(n: NodeMatch<StrDoc<SgLang>>) -> Vec<Self> {
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::verify::test::get_rule_config;

  #[test]
  fn test_generate() -> Result<()> {
    let rule_config = get_rule_config("pattern: let x = $A");
    let case = "let x = 42;";
    let result = TestSnapshot::generate(&rule_config, case)?;
    assert_eq!(
      result,
      Some(TestSnapshot {
        fixed: None,
        labels: vec![Label {
          source: "let x = 42;".into(),
          message: None,
          style: LabelStyle::Primary,
          start: 0,
          end: 11,
        }]
      })
    );
    Ok(())
  }

  #[test]
  fn test_not_found() -> Result<()> {
    let rule_config = get_rule_config("pattern: var x = $A");
    let case = "let x = 42;";
    let result = TestSnapshot::generate(&rule_config, case)?;
    assert_eq!(result, None,);
    Ok(())
  }

  #[test]
  fn test_secondary_label() -> Result<()> {
    let rule_config =
      get_rule_config("{pattern: 'let x = $A;', inside: {kind: 'statement_block'}}");
    let case = "function test() { let x = 42; }";
    let result = TestSnapshot::generate(&rule_config, case)?;
    assert_eq!(
      result,
      Some(TestSnapshot {
        fixed: None,
        labels: vec![
          Label {
            source: "let x = 42;".into(),
            message: None,
            style: LabelStyle::Primary,
            start: 18,
            end: 29,
          },
          Label {
            source: "{ let x = 42; }".into(),
            message: None,
            style: LabelStyle::Secondary,
            start: 16,
            end: 31
          }
        ],
      })
    );
    Ok(())
  }
}
