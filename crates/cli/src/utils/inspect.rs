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

use crate::config::ProjectConfig;
use crate::lang::SgLang;
use ast_grep_config::{RuleCollection, RuleConfig};

use anyhow::Result;
use clap::ValueEnum;

use std::fmt;
use std::io::{Stderr, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, ValueEnum, Default, PartialEq, Eq, PartialOrd, Ord)]
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

impl fmt::Debug for Granularity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Granularity::Nothing => write!(f, "nothing"),
      Granularity::Summary => write!(f, "summary"),
      Granularity::Entity => write!(f, "entity"),
    }
  }
}

impl Granularity {
  pub fn project_trace(&self) -> ProjectTrace {
    self.project_trace_impl(std::io::stderr())
  }
  fn project_trace_impl<W: Write>(&self, w: W) -> TraceInfo<(), W> {
    TraceInfo {
      level: *self,
      inner: (),
      output: Mutex::new(w),
    }
  }

  pub fn run_trace(&self) -> RunTrace {
    self.run_trace_impl(std::io::stderr())
  }
  fn run_trace_impl<W: Write>(&self, w: W) -> TraceInfo<FileTrace, W> {
    TraceInfo {
      level: *self,
      inner: Default::default(),
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
}

pub struct TraceInfo<T, W: Write> {
  pub level: Granularity,
  pub inner: T,
  output: Mutex<W>,
}

impl<T, W: Write + Sync> TraceInfo<T, W> {
  #[inline(always)]
  fn semi_structured_print<F>(&self, level: Granularity, f: F) -> Result<()>
  where
    F: FnOnce(&mut W) -> Result<()>,
  {
    if self.level < level {
      return Ok(());
    }
    let mut w = self.output.lock().expect("lock should not be poisoned");
    write!(w, "sg: {level:?}")?;
    f(&mut *w)?;
    writeln!(&mut *w)?;
    Ok(())
  }

  #[inline(always)]
  fn print_summary<F>(&self, entity_type: &str, kv_write: F) -> Result<()>
  where
    F: FnOnce(&mut W) -> Result<()>,
  {
    self.semi_structured_print(Granularity::Summary, |w| {
      write!(w, "|{entity_type}: ")?;
      kv_write(w)
    })
  }

  #[inline(always)]
  fn print_entity<F, D>(&self, entity_type: &str, entity_path: D, kv_write: F) -> Result<()>
  where
    F: FnOnce(&mut W) -> Result<()>,
    D: fmt::Display,
  {
    self.semi_structured_print(Granularity::Entity, |w| {
      write!(w, "|{entity_type}|{entity_path}: ")?;
      kv_write(w)
    })
  }

  fn print_files(&self, trace: &FileTrace) -> Result<()> {
    self.print_summary("file", |w| {
      let scanned = trace.files_scanned.load(Ordering::Acquire);
      let skipped = trace.files_skipped.load(Ordering::Acquire);
      write!(w, "scannedFileCount={scanned},skippedFileCount={skipped}")?;
      Ok(())
    })
  }
}

impl<W: Write + Sync> TraceInfo<(), W> {
  pub fn print_project(&self, project: &Result<ProjectConfig>) -> Result<()> {
    self.print_summary("project", |w| {
      if let Ok(project) = project {
        let dir = project.project_dir.display();
        write!(w, "isProject=true,projectDir={}", dir)?;
      } else {
        write!(w, "isProject=false")?;
      }
      Ok(())
    })
  }
}

impl<W: Write + Sync> TraceInfo<FileTrace, W> {
  pub fn print(&self) -> Result<()> {
    self.print_files(&self.inner)
  }

  pub fn print_file(&self, path: &Path, lang: SgLang) -> Result<()> {
    self.print_entity("file", path.display(), |w| {
      write!(w, "language={lang}")?;
      Ok(())
    })
  }
}

impl<W: Write + Sync> TraceInfo<RuleTrace, W> {
  // TODO: support more format?
  pub fn print(&self) -> Result<()> {
    self.print_files(&self.inner.file_trace)?;
    self.print_summary("rule", |w| {
      let (effective, skipped) = (
        self.inner.effective_rule_count,
        self.inner.skipped_rule_count,
      );
      write!(
        w,
        "effectiveRuleCount={effective},skippedRuleCount={skipped}"
      )?;
      Ok(())
    })?;
    Ok(())
  }

  pub fn print_file(&self, path: &Path, lang: SgLang, rules: &[&RuleConfig<SgLang>]) -> Result<()> {
    self.print_entity("file", path.display(), |w| {
      let len = rules.len();
      write!(w, "language={lang},appliedRuleCount={len}")?;
      Ok(())
    })?;
    Ok(())
  }

  pub fn print_rules(&self, rules: &RuleCollection<SgLang>) -> Result<()> {
    if self.level < Granularity::Entity {
      return Ok(());
    }
    rules.for_each_rule(|rule| {
      _ = self.print_entity("rule", &rule.id, |w| {
        write!(w, "finalSeverity={:?}", rule.severity)?;
        Ok(())
      });
    });
    Ok(())
  }
}

#[derive(Default)]
pub struct RuleTrace {
  pub file_trace: FileTrace,
  pub effective_rule_count: usize,
  pub skipped_rule_count: usize,
}

pub type ProjectTrace = TraceInfo<(), Stderr>;
pub type RunTrace = TraceInfo<FileTrace, Stderr>;
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
    assert_eq!(run_trace.inner.files_scanned.load(Ordering::Relaxed), 0);
    assert_eq!(run_trace.inner.files_skipped.load(Ordering::Relaxed), 0);
    assert!(run_trace.print().is_ok());
    assert_eq!(
      ret,
      "sg: summary|file: scannedFileCount=0,skippedFileCount=0\n"
    );

    let mut ret = String::new();
    let rule_stats = RuleTrace {
      effective_rule_count: 10,
      skipped_rule_count: 2,
      file_trace: Default::default(),
    };
    let scan_trace = tracing.scan_trace_impl(rule_stats, unsafe { ret.as_mut_vec() });
    assert_eq!(scan_trace.level, Granularity::Summary);
    assert_eq!(
      scan_trace
        .inner
        .file_trace
        .files_scanned
        .load(Ordering::Relaxed),
      0
    );
    assert_eq!(
      scan_trace
        .inner
        .file_trace
        .files_skipped
        .load(Ordering::Relaxed),
      0
    );
    assert_eq!(scan_trace.inner.effective_rule_count, 10);
    assert_eq!(scan_trace.inner.skipped_rule_count, 2);
    assert!(scan_trace.print().is_ok());
    assert_eq!(
      ret,
      r"sg: summary|file: scannedFileCount=0,skippedFileCount=0
sg: summary|rule: effectiveRuleCount=10,skippedRuleCount=2
"
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
