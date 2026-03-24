use crate::print::ColorArg;
use crate::utils::{prompt, run_in_alternate_screen, DiffStyles};

use ansi_term::{Color, Style};
use anyhow::Result;
use serde_yaml::to_string;

use std::io::Write;

use super::{CaseResult, CaseStatus, SnapshotAction, TestCase};

/// Represents the result of running tests
#[derive(Debug)]
pub enum TestResult {
  /// All tests passed successfully
  Success { message: String },
  /// Some tests failed due to rule errors
  RuleFail { message: String },
  /// Some tests failed due to snapshot mismatches only
  MismatchSnapshotOnly { message: String },
}

pub(super) trait Reporter {
  type Output: Write;
  fn get_output(&mut self) -> &mut Self::Output;
  fn color(&self) -> ColorArg;
  /// A hook function runs before tests start.
  fn before_report(&mut self, test_cases: &[TestCase]) -> Result<()> {
    report_case_number(self.get_output(), test_cases)
  }
  /// A hook function runs after tests completed.
  fn after_report(&mut self, results: &[CaseResult]) -> Result<TestResult> {
    let mut passed = 0;
    let mut failed = 0;

    for result in results {
      if result.passed() {
        passed += 1;
      } else {
        failed += 1;
      }
    }

    let message = format!("{passed} passed; {failed} failed;");
    if failed > 0 {
      // Check if all failing tests are snapshot mismatches only
      let all_snapshot_mismatches = results
        .iter()
        .filter(|r| !r.passed())
        .all(|r| r.is_snapshot_mismatch_only_failure());

      if all_snapshot_mismatches {
        Ok(TestResult::MismatchSnapshotOnly {
          message: format!("test failed. {message}"),
        })
      } else {
        Ok(TestResult::RuleFail {
          message: format!("test failed. {message}"),
        })
      }
    } else {
      let result = if self.color().should_use_color() {
        Color::Green.paint("ok").to_string()
      } else {
        "ok".to_string()
      };
      Ok(TestResult::Success {
        message: format!("test result: {result}. {message}"),
      })
    }
  }

  fn report_failed_cases(&mut self, results: &mut [CaseResult]) -> Result<()> {
    let output = self.get_output();
    writeln!(output)?;
    writeln!(output, "----------- Case Details -----------")?;
    for result in results {
      if result.passed() {
        continue;
      }
      for status in &mut result.cases {
        if !self.report_case_detail(result.id, status)? {
          return Ok(());
        }
      }
    }
    Ok(())
  }

  fn report_summaries(&mut self, results: &[CaseResult]) -> Result<()> {
    for result in results {
      self.report_case_summary(result.id, &result.cases)?;
    }
    let output = self.get_output();
    writeln!(output)?;
    Ok(())
  }

  fn report_case_summary(&mut self, case_id: &str, summary: &[CaseStatus]) -> Result<()> {
    let passed = summary.iter().all(CaseStatus::is_pass);
    let use_color = self.color().should_use_color();
    let case_status = if use_color {
      let style = Style::new().fg(Color::White).bold();
      if summary.is_empty() {
        style.on(Color::Yellow).paint("SKIP").to_string()
      } else if passed {
        style.on(Color::Green).paint("PASS").to_string()
      } else {
        style.on(Color::Red).paint("FAIL").to_string()
      }
    } else if summary.is_empty() {
      "SKIP".to_string()
    } else if passed {
      "PASS".to_string()
    } else {
      "FAIL".to_string()
    };
    let summary = report_summary(summary);
    writeln!(self.get_output(), "{case_status} {case_id}  {summary}")?;
    Ok(())
  }

  /// returns if should continue reporting
  /// user can mutate case_status to Updated in this function
  fn report_case_detail(&mut self, case_id: &str, result: &mut CaseStatus) -> Result<bool>;
  fn collect_snapshot_action(&self) -> SnapshotAction;
}

fn report_case_number(output: &mut impl Write, test_cases: &[TestCase]) -> Result<()> {
  writeln!(output, "Running {} tests", test_cases.len())?;
  Ok(())
}

fn report_summary(summary: &[CaseStatus]) -> String {
  if summary.len() > 40 {
    let mut pass = 0;
    let mut updated = 0;
    let mut wrong = 0;
    let mut missing = 0;
    let mut noisy = 0;
    let mut error = 0;
    for s in summary {
      match s {
        CaseStatus::Validated | CaseStatus::Reported => pass += 1,
        CaseStatus::Updated { .. } => updated += 1,
        CaseStatus::Wrong { .. } => wrong += 1,
        CaseStatus::Missing(_) => missing += 1,
        CaseStatus::Noisy(_) => noisy += 1,
        CaseStatus::Error => error += 1,
      }
    }
    let stats = vec![
      ("Pass", pass),
      ("Updated", updated),
      ("Wrong", wrong),
      ("Missing", missing),
      ("Noisy", noisy),
      ("Error", error),
    ];
    let result: Vec<_> = stats
      .into_iter()
      .filter_map(|(label, count)| {
        if count > 0 {
          Some(format!("{label} × {count}"))
        } else {
          None
        }
      })
      .collect();
    let result = result.join(", ");
    format!("{result:.^50}")
  } else {
    summary
      .iter()
      .map(|s| match s {
        CaseStatus::Validated | CaseStatus::Reported => '.',
        CaseStatus::Wrong { .. } => 'W',
        CaseStatus::Updated { .. } => 'U',
        CaseStatus::Missing(_) => 'M',
        CaseStatus::Noisy(_) => 'N',
        CaseStatus::Error => 'E',
      })
      .collect()
  }
}

fn indented_write<W: Write>(output: &mut W, code: &str) -> Result<()> {
  for line in code.lines() {
    writeln!(output, "  {line}")?;
  }
  Ok(())
}

fn report_case_detail_impl<W: Write>(
  output: &mut W,
  case_id: &str,
  result: &CaseStatus,
  color: ColorArg,
) -> Result<bool> {
  let use_color = color.should_use_color();
  let style = |s: fn(&Style) -> Style| {
    if use_color {
      s(&Style::new())
    } else {
      Style::default()
    }
  };
  let case_id = style(Style::bold).paint(case_id);
  let noisy_label = style(Style::underline).paint("Noisy");
  let missing_label = style(Style::underline).paint("Missing");
  let wrong_label = style(Style::underline).paint("Wrong");
  let error_label = style(Style::underline).paint("Error");
  let updated_label = style(Style::underline).paint("Updated");
  let diff_label = style(Style::italic).paint("Diff:");
  let snapshot_label = style(Style::italic).paint("Generated Snapshot:");
  let for_code_label = style(Style::italic).paint("For Code:");
  let styles = DiffStyles::from(color);
  match result {
    CaseStatus::Validated | CaseStatus::Reported => (),
    CaseStatus::Updated { source, .. } => {
      writeln!(
        output,
        "[{updated_label}] Rule {case_id}'s snapshot baseline has been updated."
      )?;
      writeln!(output)?;
      indented_write(output, source)?;
      writeln!(output)?;
    }
    CaseStatus::Wrong {
      source,
      actual,
      expected,
    } => {
      if let Some(expected) = expected {
        writeln!(
          output,
          "[{wrong_label}] {case_id} snapshot is different from baseline."
        )?;
        let actual_str = to_string(&actual)?;
        let expected_str = to_string(&expected)?;
        writeln!(output, "{diff_label}")?;
        styles.print_diff(&expected_str, &actual_str, output, 3)?;
      } else {
        writeln!(output, "[{wrong_label}] No {case_id} baseline found.")?;
        // TODO: add to print_styles
        writeln!(output, "{snapshot_label}")?;
        indented_write(output, &to_string(&actual)?)?;
      }
      writeln!(output, "{for_code_label}")?;
      indented_write(output, source)?;
      writeln!(output)?;
    }
    CaseStatus::Missing(s) => {
      writeln!(
        output,
        "[{missing_label}] Expect rule {case_id} to report issues, but none found in:"
      )?;
      writeln!(output)?;
      indented_write(output, s)?;
      writeln!(output)?;
    }
    CaseStatus::Noisy(s) => {
      writeln!(
        output,
        "[{noisy_label}] Expect {case_id} to report no issue, but some issues found in:"
      )?;
      writeln!(output)?;
      indented_write(output, s)?;
      writeln!(output)?;
    }
    CaseStatus::Error => {
      writeln!(output, "[{error_label}] Fail to apply fix to {case_id}")?;
    }
  }
  // continue
  Ok(true)
}

pub struct DefaultReporter<Output: Write> {
  // TODO: visibility
  pub output: Output,
  pub update_all: bool,
  pub color: ColorArg,
}

impl<O: Write> Reporter for DefaultReporter<O> {
  type Output = O;

  fn get_output(&mut self) -> &mut Self::Output {
    &mut self.output
  }
  fn color(&self) -> ColorArg {
    self.color
  }
  fn report_case_detail(&mut self, case_id: &str, result: &mut CaseStatus) -> Result<bool> {
    if self.update_all {
      result.accept();
    }
    let color = self.color;
    report_case_detail_impl(self.get_output(), case_id, result, color)
  }
  fn collect_snapshot_action(&self) -> SnapshotAction {
    if self.update_all {
      SnapshotAction::NeedUpdate
    } else {
      SnapshotAction::AcceptNone
    }
  }
}

pub struct InteractiveReporter<Output: Write> {
  pub output: Output,
  pub should_accept_all: bool,
  pub color: ColorArg,
}

const PROMPT: &str = "Accept new snapshot? (Yes[y], No[n], Accept All[a], Quit[q])";
impl<O: Write> Reporter for InteractiveReporter<O> {
  type Output = O;

  fn get_output(&mut self) -> &mut Self::Output {
    &mut self.output
  }

  fn color(&self) -> ColorArg {
    self.color
  }

  fn collect_snapshot_action(&self) -> SnapshotAction {
    SnapshotAction::NeedUpdate
  }

  fn report_case_detail(&mut self, case_id: &str, status: &mut CaseStatus) -> Result<bool> {
    if matches!(status, CaseStatus::Validated | CaseStatus::Reported) {
      return Ok(true);
    }
    let color = self.color;
    run_in_alternate_screen(|| {
      report_case_detail_impl(self.get_output(), case_id, status, color)?;
      if !matches!(status, CaseStatus::Wrong { .. }) {
        let response = prompt("Next[enter], Quit[q]", "q", Some('\n'))?;
        return Ok(response != 'q');
      }
      if self.should_accept_all {
        return self.accept_new_snapshot(status);
      }
      let response = prompt(PROMPT, "ynaq", Some('n'))?;
      match response {
        'y' => self.accept_new_snapshot(status),
        'n' => Ok(true),
        'a' => {
          self.should_accept_all = true;
          self.accept_new_snapshot(status)
        }
        'q' => Ok(false),
        _ => unreachable!(),
      }
    })
  }
}

impl<O: Write> InteractiveReporter<O> {
  fn accept_new_snapshot(&mut self, status: &mut CaseStatus) -> Result<bool> {
    let accepted = status.accept();
    debug_assert!(accepted, "status should be updated");
    Ok(true)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::verify::snapshot::TestSnapshot;
  use crate::verify::test::TEST_RULE;

  const MOCK: &str = "hello";

  fn mock_case_status() -> Vec<CaseStatus<'static>> {
    vec![
      CaseStatus::Reported,
      CaseStatus::Missing(MOCK),
      CaseStatus::Noisy(MOCK),
      CaseStatus::Wrong {
        source: MOCK,
        actual: TestSnapshot {
          fixed: None,
          labels: vec![],
        },
        expected: None,
      },
      CaseStatus::Error,
    ]
  }

  #[test]
  fn test_report_summary() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };
    reporter.report_case_summary(TEST_RULE, &mock_case_status())?;
    let s = String::from_utf8(reporter.output)?;
    assert!(s.contains(".MNWE"));
    Ok(())
  }

  #[test]
  fn test_many_cases() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };
    use std::iter::repeat_with;
    let cases: Vec<_> = repeat_with(mock_case_status).flatten().take(50).collect();
    reporter.report_case_summary(TEST_RULE, &cases)?;
    let s = String::from_utf8(reporter.output)?;
    assert!(!s.contains(".MNWE"));
    assert!(s.contains("Pass × 10, Wrong × 10, Missing × 10, Noisy × 10, Error × 10"));
    Ok(())
  }

  #[test]
  fn test_valid_case_detail() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };
    reporter.report_case_detail(TEST_RULE, &mut CaseStatus::Reported)?;
    reporter.report_case_detail(TEST_RULE, &mut CaseStatus::Validated)?;
    let s = String::from_utf8(reporter.output)?;
    assert_eq!(s, "");
    Ok(())
  }

  #[test]
  fn test_invalid_case_detail() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };
    reporter.report_case_detail(TEST_RULE, &mut CaseStatus::Missing(MOCK))?;
    reporter.report_case_detail(TEST_RULE, &mut CaseStatus::Noisy(MOCK))?;
    let s = String::from_utf8(reporter.output)?;
    assert!(s.contains("Missing"));
    assert!(s.contains("Noisy"));
    assert!(!s.contains("Error"));
    assert!(!s.contains("Wrong"));
    assert!(s.contains(MOCK));
    assert!(s.contains(TEST_RULE));
    Ok(())
  }

  #[test]
  fn test_after_report_snapshot_mismatch_only() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };

    let results = vec![CaseResult {
      id: TEST_RULE,
      cases: vec![
        CaseStatus::Wrong {
          source: MOCK,
          actual: TestSnapshot {
            fixed: None,
            labels: vec![],
          },
          expected: None,
        },
        CaseStatus::Wrong {
          source: MOCK,
          actual: TestSnapshot {
            fixed: None,
            labels: vec![],
          },
          expected: None,
        },
      ],
    }];

    let test_result = reporter.after_report(&results)?;
    assert!(matches!(
      test_result,
      TestResult::MismatchSnapshotOnly { message: _ }
    ));
    Ok(())
  }

  #[test]
  fn test_after_report_mixed_failures() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };

    let results = vec![CaseResult {
      id: TEST_RULE,
      cases: vec![
        CaseStatus::Wrong {
          source: MOCK,
          actual: TestSnapshot {
            fixed: None,
            labels: vec![],
          },
          expected: None,
        },
        CaseStatus::Missing(MOCK), // Non-snapshot failure
      ],
    }];

    let test_result = reporter.after_report(&results)?;
    assert!(matches!(test_result, TestResult::RuleFail { message: _ }));
    Ok(())
  }

  #[test]
  fn test_after_report_success() -> Result<()> {
    let output = vec![];
    let mut reporter = DefaultReporter {
      output,
      update_all: false,
      color: ColorArg::Never,
    };

    let results = vec![CaseResult {
      id: TEST_RULE,
      cases: vec![CaseStatus::Validated, CaseStatus::Reported],
    }];

    let test_result = reporter.after_report(&results)?;
    assert!(matches!(test_result, TestResult::Success { .. }));
    Ok(())
  }
}
