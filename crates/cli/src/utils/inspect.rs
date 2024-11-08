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

use anyhow::Result;
use clap::ValueEnum;

use std::io::{Stderr, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

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
    self.run_trace_impl(std::io::stderr())
  }
  fn run_trace_impl<W: Write>(&self, w: W) -> TraceInfo<(), W> {
    TraceInfo {
      level: *self,
      inner: (),
      file_trace: Default::default(),
      output: Mutex::new(w),
    }
  }

  pub fn scan_trace(&self, rule_stats: RuleTrace) -> ScanTrace {
    self.scan_trace_impl(rule_stats, std::io::stderr())
  }
  fn scan_trace_impl<W: Write>(&self, rule_stats: RuleTrace, w: W) -> TraceInfo<RuleTrace, W> {
    TraceInfo {
      level: *self,
      inner: rule_stats,
      file_trace: Default::default(),
      output: Mutex::new(w),
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
  pub fn print<W: Write>(&self, w: &mut W) -> Result<()> {
    write!(
      w,
      "Files scanned: {}, Files skipped: {}",
      self.files_scanned.load(Ordering::Acquire),
      self.files_skipped.load(Ordering::Acquire)
    )?;
    Ok(())
  }
  pub fn print_file<W: Write>(&self, w: &mut W, path: &Path, lang: SgLang) -> Result<()> {
    write!(w, "Parse {} with {lang}", path.display())?;
    Ok(())
  }
}

pub struct TraceInfo<T, W: Write> {
  pub level: Granularity,
  pub file_trace: FileTrace,
  pub inner: T,
  output: Mutex<W>,
}

impl<W: Write + Sync> TraceInfo<(), W> {
  // TODO: support more format?
  pub fn print(&self) -> Result<()> {
    match self.level {
      Granularity::Nothing => Ok(()),
      Granularity::Summary | Granularity::Entity => {
        let mut w = self.output.lock().expect("lock should not be poisoned");
        self.file_trace.print(&mut *w)?;
        writeln!(&mut *w)?;
        Ok(())
      }
    }
  }

  pub fn print_file(&self, path: &Path, lang: SgLang) -> Result<()> {
    match self.level {
      Granularity::Nothing | Granularity::Summary => Ok(()),
      Granularity::Entity => {
        let mut w = self.output.lock().expect("lock should not be poisoned");
        self.file_trace.print_file(&mut *w, path, lang)?;
        writeln!(&mut *w)?;
        Ok(())
      }
    }
  }
}

impl<W: Write> TraceInfo<RuleTrace, W> {
  // TODO: support more format?
  pub fn print(&self) -> Result<()> {
    match self.level {
      Granularity::Nothing => Ok(()),
      Granularity::Summary | Granularity::Entity => {
        let mut w = self.output.lock().expect("lock should not be poisoned");
        self.file_trace.print(&mut *w)?;
        writeln!(&mut *w, "\n{}", self.inner.print())?;
        Ok(())
      }
    }
  }
  pub fn print_file(&self, path: &Path, lang: SgLang, rules: &[&RuleConfig<SgLang>]) -> Result<()> {
    let len = rules.len();
    match self.level {
      Granularity::Nothing | Granularity::Summary => Ok(()),
      Granularity::Entity => {
        let mut w = self.output.lock().expect("lock should not be poisoned");
        self.file_trace.print_file(&mut *w, path, lang)?;
        writeln!(&mut *w, ", applied {len} rule(s)")?;
        Ok(())
      }
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

pub type RunTrace = TraceInfo<(), Stderr>;
pub type ScanTrace = TraceInfo<RuleTrace, Stderr>;

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_tracing() {
    let tracing = Granularity::Summary;
    let mut ret = String::new();
    let run_trace = tracing.run_trace_impl(unsafe { ret.as_mut_vec() });
    assert_eq!(run_trace.level, Granularity::Summary);
    assert_eq!(
      run_trace.file_trace.files_scanned.load(Ordering::Relaxed),
      0
    );
    assert_eq!(
      run_trace.file_trace.files_skipped.load(Ordering::Relaxed),
      0
    );
    assert!(run_trace.print().is_ok());
    assert_eq!(ret, "Files scanned: 0, Files skipped: 0\n");

    let mut ret = String::new();
    let rule_stats = RuleTrace {
      effective_rule_count: 10,
      skipped_rule_count: 2,
    };
    let scan_trace = tracing.scan_trace_impl(rule_stats, unsafe { ret.as_mut_vec() });
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
    assert!(scan_trace.print().is_ok());
    assert_eq!(
      ret,
      "Files scanned: 0, Files skipped: 0\nEffective rules: 10, Skipped rules: 2\n"
    );
  }

  #[test]
  fn test_tracing_nothing() {
    let tracing = Granularity::Nothing;
    let mut ret = String::new();
    let run_trace = tracing.run_trace_impl(unsafe { ret.as_mut_vec() });
    assert_eq!(run_trace.level, Granularity::Nothing);
    assert!(run_trace.print().is_ok());
    assert!(ret.is_empty());
  }
}
