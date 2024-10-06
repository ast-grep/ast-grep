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

#[derive(Clone, Copy, ValueEnum, Serialize, Deserialize, Default, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
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
  pub fn run_trace(&self) -> RunTrace {
    RunTrace {
      level: *self,
      inner: (),
      file_trace: Default::default(),
    }
  }
  pub fn scan_trace(&self, rule_stats: RuleTrace) -> ScanTrace {
    ScanTrace {
      level: *self,
      inner: rule_stats,
      file_trace: Default::default(),
    }
  }
}

// total = scanned + skipped
//       = (matched + unmatched) + skipped
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTrace {
  files_scanned: AtomicUsize,
  files_skipped: AtomicUsize,
}

impl FileTrace {
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
#[serde(rename_all = "camelCase")]
pub struct TraceInfo<T> {
  pub level: Tracing,
  #[serde(flatten)]
  pub file_trace: FileTrace,
  #[serde(flatten)]
  pub inner: T,
}
impl TraceInfo<()> {
  // TODO: support more format?
  pub fn print(&self, is_json: bool) -> Option<String> {
    if self.level == Tracing::Nothing {
      None
    } else if is_json {
      Some(serde_json::to_string(self).ok()?)
    } else {
      Some(self.file_trace.print())
    }
  }
}

impl TraceInfo<RuleTrace> {
  // TODO: support more format?
  pub fn print(&self, is_json: bool) -> Option<String> {
    if self.level == Tracing::Nothing {
      None
    } else if is_json {
      Some(serde_json::to_string(self).ok()?)
    } else {
      Some(format!(
        "{}\n{}",
        self.file_trace.print(),
        self.inner.print()
      ))
    }
  }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleTrace {
  pub effective_rule_count: usize,
  pub skipped_rule_count: usize,
}
impl RuleTrace {
  pub fn print(&self) -> String {
    format!(
      "Effective rules: {}, Skipped rules: {}",
      self.effective_rule_count, self.skipped_rule_count
    )
  }
}

pub type RunTrace = TraceInfo<()>;
pub type ScanTrace = TraceInfo<RuleTrace>;

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_tracing() {
    let tracing = Tracing::Summary;
    let run_trace = tracing.run_trace();
    assert_eq!(run_trace.level, Tracing::Summary);
    assert_eq!(
      run_trace.file_trace.files_scanned.load(Ordering::Relaxed),
      0
    );
    assert_eq!(
      run_trace.file_trace.files_skipped.load(Ordering::Relaxed),
      0
    );
    let printed = run_trace.print(false).expect("should have output");
    assert_eq!(printed, "Files scanned: 0, Files skipped: 0");

    let rule_stats = RuleTrace {
      effective_rule_count: 10,
      skipped_rule_count: 2,
    };
    let scan_trace = tracing.scan_trace(rule_stats);
    assert_eq!(scan_trace.level, Tracing::Summary);
    assert_eq!(
      scan_trace.file_trace.files_scanned.load(Ordering::Relaxed),
      0
    );
    assert_eq!(
      scan_trace.file_trace.files_skipped.load(Ordering::Relaxed),
      0
    );
    assert_eq!(scan_trace.inner.effective_rule_count, 10);
    assert_eq!(scan_trace.inner.skipped_rule_count, 2);
    let printed = scan_trace.print(false).expect("should have output");
    assert_eq!(
      printed,
      "Files scanned: 0, Files skipped: 0\nEffective rules: 10, Skipped rules: 2"
    );
  }

  #[test]
  fn test_tracing_nothing() {
    let tracing = Tracing::Nothing;
    let run_trace = tracing.run_trace();
    assert_eq!(run_trace.level, Tracing::Nothing);
    let printed = run_trace.print(false);
    assert!(printed.is_none());
  }
}
