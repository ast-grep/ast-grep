//! See https://github.com/ast-grep/ast-grep/issues/905
//! The `--tracing` flag helps user to inspect ast-grep's scan process. It has these levels:
//! - Summary level: show how many files are scanned, how many matches and etc for one CLI run. Included stats
//!   * number of rules used in this scan and skipped rules (due to severity: off)
//!   * number file scanned
//!   * number file matched
//! - Entity level: show how a file is scanned
//!   * reasons if skipped (file too large, does not have fixed string in pattern, no matching rule, etc)
//!   * number of rules applied
//!   * rules skipped (dues to ignore/files)
//! - Detail level: show how a rule runs on a file

use crate::lang::SgLang;
use ast_grep_config::RuleConfig;

use clap::ValueEnum;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, ValueEnum, Default, PartialEq, Debug)]
pub enum Granularity {
  /// Do not show any tracing information
  #[default]
  Nothing = 0,
  /// Show summary about how many files are scanned and skipped
  Summary = 1,
  /// Show per-file/per-rule tracing information
  Entity = 2,
  // Detail,
}

impl Granularity {
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
#[derive(Default)]
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
  pub fn print_file(&self, path: &Path, lang: SgLang) -> String {
    format!("Parse {} with {lang}", path.display())
  }
}

pub struct TraceInfo<T> {
  pub level: Granularity,
  pub file_trace: FileTrace,
  pub inner: T,
}

impl TraceInfo<()> {
  // TODO: support more format?
  pub fn print(&self) -> Option<String> {
    match self.level {
      Granularity::Nothing => None,
      Granularity::Summary | Granularity::Entity => Some(self.file_trace.print()),
    }
  }

  pub fn print_file(&self, path: &Path, lang: SgLang) -> Option<String> {
    match self.level {
      Granularity::Nothing => None,
      Granularity::Summary => None,
      Granularity::Entity => Some(self.file_trace.print_file(path, lang)),
    }
  }
}

impl TraceInfo<RuleTrace> {
  // TODO: support more format?
  pub fn print(&self) -> Option<String> {
    match self.level {
      Granularity::Nothing => None,
      Granularity::Summary | Granularity::Entity => Some(format!(
        "{}\n{}",
        self.file_trace.print(),
        self.inner.print()
      )),
    }
  }
  pub fn print_file(
    &self,
    path: &Path,
    lang: SgLang,
    rules: &[&RuleConfig<SgLang>],
  ) -> Option<String> {
    let len = rules.len();
    match self.level {
      Granularity::Nothing | Granularity::Summary => None,
      Granularity::Entity => Some(format!(
        "{}, applied {len} rule(s)",
        self.file_trace.print_file(path, lang),
      )),
    }
  }
}

#[derive(Default)]
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
    let tracing = Granularity::Summary;
    let run_trace = tracing.run_trace();
    assert_eq!(run_trace.level, Granularity::Summary);
    assert_eq!(
      run_trace.file_trace.files_scanned.load(Ordering::Relaxed),
      0
    );
    assert_eq!(
      run_trace.file_trace.files_skipped.load(Ordering::Relaxed),
      0
    );
    let printed = run_trace.print().expect("should have output");
    assert_eq!(printed, "Files scanned: 0, Files skipped: 0");

    let rule_stats = RuleTrace {
      effective_rule_count: 10,
      skipped_rule_count: 2,
    };
    let scan_trace = tracing.scan_trace(rule_stats);
    assert_eq!(scan_trace.level, Granularity::Summary);
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
    let printed = scan_trace.print().expect("should have output");
    assert_eq!(
      printed,
      "Files scanned: 0, Files skipped: 0\nEffective rules: 10, Skipped rules: 2"
    );
  }

  #[test]
  fn test_tracing_nothing() {
    let tracing = Granularity::Nothing;
    let run_trace = tracing.run_trace();
    assert_eq!(run_trace.level, Granularity::Nothing);
    let printed = run_trace.print();
    assert!(printed.is_none());
  }
}
