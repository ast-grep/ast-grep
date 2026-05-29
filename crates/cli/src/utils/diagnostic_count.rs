use ast_grep_config::Severity;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default, Clone, Copy, Debug)]
pub struct DiagnosticSnapshot {
  pub errors: usize,
  pub warnings: usize,
  pub infos: usize,
  pub hints: usize,
}

impl DiagnosticSnapshot {
  pub fn add(&mut self, severity: &Severity, n: usize) {
    match severity {
      Severity::Error => self.errors = self.errors.saturating_add(n),
      Severity::Warning => self.warnings = self.warnings.saturating_add(n),
      Severity::Info => self.infos = self.infos.saturating_add(n),
      Severity::Hint => self.hints = self.hints.saturating_add(n),
      Severity::Off => {}
    }
  }

  pub fn total(&self) -> usize {
    self
      .errors
      .saturating_add(self.warnings)
      .saturating_add(self.infos)
      .saturating_add(self.hints)
  }
}

#[derive(Default)]
pub struct DiagnosticCount {
  error: AtomicUsize,
  warning: AtomicUsize,
  info: AtomicUsize,
  hint: AtomicUsize,
}

impl DiagnosticCount {
  pub fn merge(&self, local: DiagnosticSnapshot) {
    self.error.fetch_add(local.errors, Ordering::AcqRel);
    self.warning.fetch_add(local.warnings, Ordering::AcqRel);
    self.info.fetch_add(local.infos, Ordering::AcqRel);
    self.hint.fetch_add(local.hints, Ordering::AcqRel);
  }

  pub fn snapshot(&self) -> DiagnosticSnapshot {
    DiagnosticSnapshot {
      errors: self.error.load(Ordering::Acquire),
      warnings: self.warning.load(Ordering::Acquire),
      infos: self.info.load(Ordering::Acquire),
      hints: self.hint.load(Ordering::Acquire),
    }
  }
}
