/*!
Maintains struct/enum for rule test results.

ast-grep rule test has several concepts.
Refer to https://ast-grep.github.io/guide/test-rule.html#basic-concepts
for general review.
*/
use super::{snapshot::TestSnapshot, SgLang, TestSnapshots};
use ast_grep_config::RuleConfig;
use ast_grep_language::Language;

/// [CaseStatus] categorize whether and how ast-grep
/// reports error for either valid or invalid code.
///
/// TestCase has two forms of input: valid code and invalid code.
/// sg can either reports or not reports an error.
/// This is a 2*2 = 4 scenarios. Also for reported scenario, we may have snapshot mismatching.
#[derive(PartialEq, Eq, Debug)]
pub enum CaseStatus<'a> {
  /// Reported no issue for valid code
  Validated,
  /// Reported correct issue for invalid code
  Reported,
  /// User accepted new snapshot updates
  Updated {
    source: &'a str,
    updated: TestSnapshot,
  },
  /// Reported issues for invalid code but it is wrong
  Wrong {
    source: &'a str,
    actual: TestSnapshot,
    expected: Option<TestSnapshot>,
  },
  /// Reported no issue for invalid code
  Missing(&'a str),
  /// Reported some issue for valid code
  Noisy(&'a str),
  /// Error occurred when applying fix
  Error,
}

impl<'a> CaseStatus<'a> {
  pub fn verify_valid(rule_config: &RuleConfig<SgLang>, case: &'a str) -> Self {
    let rule = &rule_config.matcher;
    let sg = rule_config.language.ast_grep(case);
    if sg.root().find(rule).is_some() {
      CaseStatus::Noisy(case)
    } else {
      CaseStatus::Validated
    }
  }

  pub fn verify_invalid(rule_config: &RuleConfig<SgLang>, case: &'a str) -> Self {
    let sg = rule_config.language.ast_grep(case);
    let rule = &rule_config.matcher;
    if sg.root().find(rule).is_some() {
      CaseStatus::Reported
    } else {
      CaseStatus::Missing(case)
    }
  }

  pub fn verify_snapshot(
    rule_config: &RuleConfig<SgLang>,
    case: &'a str,
    snapshot: Option<&TestSnapshot>,
  ) -> Self {
    let actual = match TestSnapshot::generate(rule_config, case) {
      Ok(Some(snap)) => snap,
      Ok(None) => return CaseStatus::Missing(case),
      Err(_) => return CaseStatus::Error,
    };
    match snapshot {
      Some(e) if e == &actual => CaseStatus::Reported,
      nullable => CaseStatus::Wrong {
        source: case,
        actual,
        expected: nullable.cloned(),
      },
    }
  }

  pub fn accept(&mut self) -> bool {
    let CaseStatus::Wrong { source, actual, .. } = self else {
      return false;
    };
    let updated = std::mem::replace(
      actual,
      TestSnapshot {
        fixed: None,
        labels: vec![],
      },
    );
    *self = CaseStatus::Updated { source, updated };
    true
  }

  pub fn is_pass(&self) -> bool {
    matches!(
      self,
      CaseStatus::Validated | CaseStatus::Reported | CaseStatus::Updated { .. }
    )
  }
}

/// The result for one rule-test.yml
/// id is the rule id. cases contains a list of [CaseStatus] for valid and invalid cases.
#[derive(PartialEq, Eq, Default, Debug)]
pub struct CaseResult<'a> {
  pub id: &'a str,
  pub cases: Vec<CaseStatus<'a>>,
}

impl CaseResult<'_> {
  /// Did all cases in the rule-test pass the test?
  pub fn passed(&self) -> bool {
    self.cases.iter().all(CaseStatus::is_pass)
  }
  pub fn changed_snapshots(&self) -> TestSnapshots {
    let snapshots = self
      .cases
      .iter()
      .filter_map(|c| match c {
        CaseStatus::Updated { source, updated } => Some((source.to_string(), updated.clone())),
        _ => None,
      })
      .collect();
    TestSnapshots {
      id: self.id.to_string(),
      snapshots,
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::verify::test::get_rule_config;

  #[test]
  fn test_snapshot() {
    let rule = get_rule_config("pattern: let a = 1");
    let ret = CaseStatus::verify_snapshot(&rule, "function () { let a = 1 }", None);
    assert!(matches!(&ret, CaseStatus::Wrong { expected: None, .. }));
    let CaseStatus::Wrong { actual, source, .. } = ret else {
      panic!("wrong");
    };
    assert_eq!(source, "function () { let a = 1 }");
    let primary = &actual.labels[0];
    assert_eq!(primary.source, "let a = 1");
    let ret = CaseStatus::verify_snapshot(&rule, "function () { let a = 1 }", Some(&actual));
    assert!(matches!(ret, CaseStatus::Reported));
  }
}
