//! See https://github.com/ast-grep/ast-grep/issues/905
//! The `--tracing` flag helps user to inspect ast-grep's scan process. It has these levels:
//! - Summary level: show how many files are scanned, how many matches and etc for one CLI run. Included stats
//!   * number of rules used in this scan and skipped rules (due to severity: off)
//!   * number file scanned
//!   * number file matched
//!   * number of matches produced or errors/warnings/hints
//!   * number of fix applied
//! - File level: show how a file is scanned
//!   * reasons if skipped (file too large, does not have fixed string in pattern, no matching rule, etc)
//!   * number of rules applied
//!   * rules skipped (dues to ignore/files)
//!   * matches produced or errors/warnings/hints
//!   * number of fix applied
//! - Rule level: show how a rule scans files
//!   * number of files applied
//!   * matches produced or errors/warnings/hints
//!   * number of fix applied
//! - Detail level: show how a rule runs on a file

use ast_grep_config::Severity;
use clap::ValueEnum;

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, ValueEnum)]
pub enum Tracing {
  Summary,
  // TODO: implement these levels
  // File,
  // Rule,
  // Detail,
}

// total = scanned + skipped
//       = (matched + unmatched) + skipped
#[derive(Default)]
struct FileStats {
  files_scanned: AtomicUsize,
  files_skipped: AtomicUsize,
}

impl FileStats {
  // AcqRel is stronger than needed. Relaxed is enough.
  pub fn add_scanned(&self) {
    self.files_scanned.fetch_add(1, Ordering::AcqRel);
  }
  pub fn add_skipped(&self) {
    self.files_skipped.fetch_add(1, Ordering::AcqRel);
  }
  pub fn get_scanned(&self) -> usize {
    self.files_scanned.load(Ordering::Acquire)
  }
  pub fn get_skipped(&self) -> usize {
    self.files_skipped.load(Ordering::Acquire)
  }
}

struct SummaryStats<T> {
  pub file_stats: FileStats,
  pub inner: T,
  pub fix_applied: usize,
}

impl<T> Deref for SummaryStats<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> DerefMut for SummaryStats<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

// these do not need to be atomic since they are only accessed by one thread
#[derive(Default)]
struct PatternStats {
  pub matched: usize,
}

#[derive(Default)]
struct RuleStats {
  pub effective_rule_count: usize,
  pub skipped_rule_count: usize,
  pub errors: usize,
  pub warnings: usize,
  pub infos: usize,
  pub hints: usize,
}

impl RuleStats {
  pub fn add_effective_rule(&mut self) {
    self.effective_rule_count += 1;
  }
  pub fn add_skipped_rule(&mut self) {
    self.skipped_rule_count += 1;
  }
  pub fn count_match(&mut self, severity: Severity) {
    match severity {
      Severity::Error => self.errors += 1,
      Severity::Warning => self.warnings += 1,
      Severity::Info => self.infos += 1,
      Severity::Hint => self.hints += 1,
      Severity::Off => unreachable!("off rule should not have match"),
    }
  }
}

pub type RunStats = SummaryStats<PatternStats>;
pub type ScanStats = SummaryStats<RuleStats>;

impl<T: Default> Default for SummaryStats<T> {
  fn default() -> Self {
    Self {
      file_stats: FileStats::default(),
      inner: T::default(),
      fix_applied: 0,
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_file_stats() {
    let stats = FileStats::default();
    assert_eq!(stats.get_scanned(), 0);
    assert_eq!(stats.get_skipped(), 0);

    stats.add_skipped();
    assert_eq!(stats.get_scanned(), 0);
    assert_eq!(stats.get_skipped(), 1);

    stats.add_scanned();
    assert_eq!(stats.get_scanned(), 1);
    assert_eq!(stats.get_skipped(), 1);
  }
}
