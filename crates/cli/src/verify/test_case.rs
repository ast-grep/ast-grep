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
    verify_test_case(self, rule_config)
  }

  pub fn verify_with_snapshot(
    &self,
    rule_config: &RuleConfig<SgLang>,
    snapshots: Option<&TestSnapshots>,
  ) -> CaseResult {
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
