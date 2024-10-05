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
//! - Detail level: show how a rule runs on a file

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, ValueEnum, Serialize, Deserialize, Default, PartialEq)]
pub enum Tracing {
  /// Do not show any tracing information
  #[default]
  Nothing = 0,
  /// Show summary about how many files are scanned and skipped
  Summary = 1,
  // TODO: implement these levels
  // File,
  // Detail,
}

impl Tracing {
  pub fn run_stats(&self) -> RunStats {
    RunStats {
      level: *self,
      inner: (),
      file_stats: Default::default(),
    }
  }
  pub fn scan_stats(&self, rule_stats: RuleStats) -> ScanStats {
    ScanStats {
      level: *self,
      inner: rule_stats,
      file_stats: Default::default(),
    }
  }
}

// total = scanned + skipped
//       = (matched + unmatched) + skipped
#[derive(Default, Serialize, Deserialize)]
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
  pub fn print(&self) -> String {
    format!(
      "Files scanned: {}, Files skipped: {}",
      self.files_scanned.load(Ordering::Acquire),
      self.files_skipped.load(Ordering::Acquire)
    )
  }
}

#[derive(Serialize, Deserialize)]
pub struct SummaryStats<T> {
  pub level: Tracing,
  pub file_stats: FileStats,
  #[serde(flatten)]
  pub inner: T,
}
impl SummaryStats<()> {
  pub fn print(&self) -> Option<String> {
    if self.level == Tracing::Nothing {
      return None;
    }
    Some(self.file_stats.print())
  }
}

impl SummaryStats<RuleStats> {
  pub fn print(&self) -> Option<String> {
    if self.level == Tracing::Nothing {
      return None;
    }
    Some(format!(
      "{}\n{}",
      self.file_stats.print(),
      self.inner.print()
    ))
  }
}

#[derive(Default, Serialize, Deserialize)]
pub struct RuleStats {
  pub effective_rule_count: usize,
  pub skipped_rule_count: usize,
}
impl RuleStats {
  pub fn print(&self) -> String {
    format!(
      "Effective rules: {}, Skipped rules: {}",
      self.effective_rule_count, self.skipped_rule_count
    )
  }
}

pub type RunStats = SummaryStats<()>;
pub type ScanStats = SummaryStats<RuleStats>;
