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

use clap::ValueEnum;

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
#[derive(Default, Debug)]
pub struct FileStats {
  files_scanned: AtomicUsize,
  files_skipped: AtomicUsize,
}

impl FileStats {
  pub fn add_scanned(&self) {
    self.files_scanned.fetch_add(1, Ordering::AcqRel);
  }
  pub fn add_skipped(&self) {
    self.files_skipped.fetch_add(1, Ordering::AcqRel);
  }
  pub fn scanned(&self) -> usize {
    self.files_scanned.load(Ordering::Acquire)
  }
  pub fn skipped(&self) -> usize {
    self.files_skipped.load(Ordering::Acquire)
  }
}

#[derive(Debug)]
pub struct SummaryStats<T> {
  pub file_stats: FileStats,
  pub inner: T,
}

#[derive(Default, Debug)]
pub struct RuleStats {
  pub effective_rule_count: usize,
  pub skipped_rule_count: usize,
}

pub type RunStats = SummaryStats<()>;
pub type ScanStats = SummaryStats<RuleStats>;

impl<T: Default> Default for SummaryStats<T> {
  fn default() -> Self {
    Self {
      file_stats: FileStats::default(),
      inner: T::default(),
    }
  }
}
