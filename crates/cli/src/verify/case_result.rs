/*!
Maintains struct/enum for rule test results.

ast-grep rule test has several concepts.
Refer to https://ast-grep.github.io/guide/test-rule.html#basic-concepts
for general review.
*/
use super::{SnapshotCollection, TestSnapshot, TestSnapshots};

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

/// The result for one rule-test.yml
/// id is the rule id. cases contains a list of [CaseStatus] for valid and invalid cases.
#[derive(PartialEq, Eq, Default, Debug)]
pub struct CaseResult<'a> {
  pub id: &'a str,
  pub cases: Vec<CaseStatus<'a>>,
}

impl<'a> CaseResult<'a> {
  /// Did all cases in the rule-test pass the test?
  pub fn passed(&self) -> bool {
    self
      .cases
      .iter()
      .all(|c| matches!(c, CaseStatus::Validated | CaseStatus::Reported))
  }
  pub fn changed_snapshots(&self) -> TestSnapshots {
    let snapshots = self
      .cases
      .iter()
      .filter_map(|c| match c {
        CaseStatus::Wrong { source, actual, .. } => Some((source.to_string(), actual.clone())),
        _ => None,
      })
      .collect();
    TestSnapshots {
      id: self.id.to_string(),
      snapshots,
    }
  }
}
