use ast_grep_config::Severity;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Per-file diagnostic counts, accumulated locally before one shared merge.
/// Only errors and warnings appear in the final summary, so only they are tracked.
#[derive(Default)]
pub struct DiagnosticSnapshot {
  pub errors: usize,
  pub warnings: usize,
}

impl DiagnosticSnapshot {
  pub fn add(&mut self, severity: &Severity, n: usize) {
    match severity {
      Severity::Error => self.errors = self.errors.saturating_add(n),
      Severity::Warning => self.warnings = self.warnings.saturating_add(n),
      Severity::Info | Severity::Hint | Severity::Off => {}
    }
  }
}

/// Shared counts merged from parallel scan workers.
#[derive(Default)]
pub struct DiagnosticCount {
  error: AtomicUsize,
  warning: AtomicUsize,
}

impl DiagnosticCount {
  pub fn merge(&self, local: DiagnosticSnapshot) {
    // most files have no diagnostics, skip the atomic write for them
    if local.errors > 0 {
      self.error.fetch_add(local.errors, Ordering::AcqRel);
    }
    if local.warnings > 0 {
      self.warning.fetch_add(local.warnings, Ordering::AcqRel);
    }
  }

  pub fn errors(&self) -> usize {
    self.error.load(Ordering::Acquire)
  }

  pub fn warnings(&self) -> usize {
    self.warning.load(Ordering::Acquire)
  }
}
