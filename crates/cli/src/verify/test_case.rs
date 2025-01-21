use super::case_result::{CaseResult, CaseStatus};
use super::snapshot::TestSnapshots;
use crate::lang::SgLang;

use ast_grep_config::RuleConfig;
use serde::{Deserialize, Serialize};

/// Corresponds to one rule-test.yml for testing.
///
/// A rule-test contains these fields:
/// * id: the id of the rule that will be tested against
/// * valid: code that we do not expect to have any issues
/// * invalid: code that we do expect to have some issues
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
}

impl TestCase {
  pub fn verify_rule(&self, rule_config: &RuleConfig<SgLang>) -> CaseResult {
    debug_assert_eq!(self.id, rule_config.id);
    verify_test_case(self, rule_config)
  }

  pub fn verify_with_snapshot(
    &self,
    rule_config: &RuleConfig<SgLang>,
    snapshots: Option<&TestSnapshots>,
  ) -> CaseResult {
    debug_assert_eq!(self.id, rule_config.id);
    verify_test_case_with_snapshots(self, rule_config, snapshots)
  }
}

fn verify_test_case<'a>(
  test_case: &'a TestCase,
  rule_config: &RuleConfig<SgLang>,
) -> CaseResult<'a> {
  let valid_cases = test_case
    .valid
    .iter()
    .map(|valid| CaseStatus::verify_valid(rule_config, valid));
  let invalid_cases = test_case
    .invalid
    .iter()
    .map(|invalid| CaseStatus::verify_invalid(rule_config, invalid));
  CaseResult {
    id: &test_case.id,
    cases: valid_cases.chain(invalid_cases).collect(),
  }
}

fn verify_test_case_with_snapshots<'a>(
  test_case: &'a TestCase,
  rule_config: &RuleConfig<SgLang>,
  snapshots: Option<&TestSnapshots>,
) -> CaseResult<'a> {
  let valid_cases = test_case
    .valid
    .iter()
    .map(|valid| CaseStatus::verify_valid(rule_config, valid));
  let invalid_cases = test_case.invalid.iter().map(|invalid| {
    let snap = snapshots.and_then(|s| s.snapshots.get(invalid));
    CaseStatus::verify_snapshot(rule_config, invalid, snap)
  });
  CaseResult {
    id: &test_case.id,
    cases: valid_cases.chain(invalid_cases).collect(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::verify::snapshot::TestSnapshot;
  use crate::verify::test::{get_rule_config, TEST_RULE};

  fn mock_test_case(valid: &[&str], invalid: &[&str]) -> TestCase {
    TestCase {
      id: TEST_RULE.to_string(),
      valid: valid.iter().map(|s| s.to_string()).collect(),
      invalid: invalid.iter().map(|s| s.to_string()).collect(),
    }
  }

  fn mock_snapshots(case: &str, snap: TestSnapshot) -> TestSnapshots {
    TestSnapshots {
      id: TEST_RULE.to_string(),
      snapshots: vec![(case.to_string(), snap)].into_iter().collect(),
    }
  }

  fn mock_snapshot(rule_config: &RuleConfig<SgLang>, case: &str) -> TestSnapshot {
    TestSnapshot::generate(rule_config, case)
      .expect("should ok")
      .expect("should generate")
  }

  #[test]
  fn test_verify_rule() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = mock_test_case(&["var x = 123"], &["let x = 123"]);
    let result = test_case.verify_rule(&rule_config);
    assert_eq!(result.id, test_case.id);
    assert!(matches!(result.cases[0], CaseStatus::Validated));
    assert!(matches!(result.cases[1], CaseStatus::Reported));
  }

  #[test]
  fn test_invalid() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = mock_test_case(&["let x = 123"], &["var x = 123"]);
    let result = test_case.verify_rule(&rule_config);
    assert_eq!(result.id, test_case.id);
    assert!(matches!(result.cases[0], CaseStatus::Noisy("let x = 123")));
    assert!(matches!(
      result.cases[1],
      CaseStatus::Missing("var x = 123")
    ));
  }
  #[test]
  fn test_verify_snapshot_with_existing() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = mock_test_case(&[], &["let x = 123"]);
    let snap = mock_snapshot(&rule_config, "let x = 123");
    let snaps = mock_snapshots("let x = 123", snap.clone());
    let result = test_case.verify_with_snapshot(&rule_config, Some(&snaps));
    assert_eq!(result.cases[0], CaseStatus::Reported);
  }

  #[test]
  fn test_verify_snapshot_with_mismatch() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = mock_test_case(&["var x = 123"], &["let x = 123"]);
    let snap = mock_snapshot(&rule_config, "let x = 456");
    let snaps = mock_snapshots("let x = 123", snap.clone());
    let result = test_case.verify_with_snapshot(&rule_config, Some(&snaps));
    assert_eq!(result.cases[0], CaseStatus::Validated);
    assert_eq!(
      result.cases[1],
      CaseStatus::Wrong {
        source: "let x = 123",
        actual: mock_snapshot(&rule_config, "let x = 123"),
        expected: Some(mock_snapshot(&rule_config, "let x = 456")),
      }
    );
  }

  #[test]
  fn test_verify_snapshot_without_existing() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = mock_test_case(&["var x = 123"], &["let x = 123"]);
    let result = test_case.verify_with_snapshot(&rule_config, None);
    assert_eq!(result.cases[0], CaseStatus::Validated);
    assert_eq!(
      result.cases[1],
      CaseStatus::Wrong {
        source: "let x = 123",
        actual: mock_snapshot(&rule_config, "let x = 123"),
        expected: None,
      }
    );
  }

  #[test]
  fn test_verify_snapshot_without_existing_2() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = mock_test_case(&["var x = 123"], &["let x = 123"]);
    let snap = mock_snapshot(&rule_config, "let x = 456");
    let snaps = mock_snapshots("let x = 456", snap.clone());
    let result = test_case.verify_with_snapshot(&rule_config, Some(&snaps));
    assert_eq!(result.cases[0], CaseStatus::Validated);
    assert_eq!(
      result.cases[1],
      CaseStatus::Wrong {
        source: "let x = 123",
        actual: mock_snapshot(&rule_config, "let x = 123"),
        expected: None,
      }
    );
  }

  #[test]
  #[should_panic]
  #[cfg(debug_assertions)]
  fn test_unmatching_id() {
    let rule_config = get_rule_config("pattern: let x = $A");
    let test_case = TestCase {
      id: "non-matching".into(),
      valid: vec![],
      invalid: vec![],
    };
    test_case.verify_rule(&rule_config);
  }
}
