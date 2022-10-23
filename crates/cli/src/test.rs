use crate::config::{find_config, find_tests, read_test_files};
use crate::languages::{Language, SupportLang};
use ansi_term::{Color, Style};
use anyhow::Result;
use ast_grep_config::RuleCollection;
use clap::Args;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum LabelStyle {
  Primary,
  Secondary,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Label {
  source: String,
  message: Option<String>,
  style: LabelStyle,
  start: usize,
  end: usize,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TestSnapshot {
  pub id: String,
  pub source: String,
  pub fixed: Option<String>,
  pub labels: Vec<Label>,
}

#[derive(Args)]
pub struct TestArg {
  /// Path to the root ast-grep config YAML
  #[clap(short, long)]
  config: Option<PathBuf>,
  /// the directories to search test YAML files
  #[clap(short, long)]
  test_dir: Option<PathBuf>,
  /// Specify the directory name storing snapshots. Default to __snapshots__.
  #[clap(long)]
  snapshot_dir: Option<PathBuf>,
  /// Only check if the code in a test case is valid code or not.
  /// Turn it on when you want to ignore the output of rules.
  #[clap(long, default_value = "true")]
  simple: bool,
  /// Update the content of all snapshots that have changed in test.
  #[clap(short, long)]
  update_snapshots: bool,
  /// start an interactive session to update snapshots selectively
  #[clap(short, long)]
  interactive: bool,
}

pub fn run_test_rule(arg: TestArg) -> Result<()> {
  run_test_rule_impl(arg, std::io::stdout())
}

fn run_test_rule_impl(arg: TestArg, output: impl Write) -> Result<()> {
  let collections = find_config(arg.config.clone())?;
  let (test_cases, _snapshots) = if let Some(test_dir) = arg.test_dir {
    let base_dir = std::env::current_dir()?;
    let snapshot_dir = arg.snapshot_dir.as_deref();
    read_test_files(&base_dir, &test_dir, snapshot_dir)?
  } else {
    find_tests(arg.config)?
  };
  let mut reporter = DefaultReporter { output };
  reporter.before_report(&test_cases)?;
  let mut results = vec![];
  for case in &test_cases {
    match verify_test_case_simple(&collections, case) {
      Some(result) => {
        reporter.report_case_summary(&case.id, &result.cases)?;
        results.push(result);
      }
      None => {
        let output = &mut reporter.output;
        writeln!(output, "Configuraiont not found! {}", case.id)?;
      }
    }
  }
  reporter.report_failed_cases(&results)?;
  reporter.after_report(&results)?;
  Ok(())
}

fn verify_test_case_simple<'a>(
  rules: &RuleCollection<SupportLang>,
  test_case: &'a TestCase,
) -> Option<CaseResult<'a>> {
  let rule = rules.get_rule(&test_case.id)?;
  let lang = rule.language;
  let rule = rule.get_rule();
  let valid_cases = test_case.valid.iter().map(|valid| {
    let sg = lang.ast_grep(valid);
    if sg.root().find(&rule).is_some() {
      CaseStatus::Noisy(valid)
    } else {
      CaseStatus::Validated
    }
  });
  let invalid_cases = test_case.invalid.iter().map(|invalid| {
    let sg = lang.ast_grep(invalid);
    if sg.root().find(&rule).is_none() {
      CaseStatus::Missing(invalid)
    } else {
      CaseStatus::Reported
    }
  });
  Some(CaseResult {
    id: &test_case.id,
    cases: valid_cases.chain(invalid_cases).collect(),
  })
}

#[derive(PartialEq, Eq, Default, Debug)]
struct CaseResult<'a> {
  id: &'a str,
  cases: Vec<CaseStatus<'a>>,
}

impl<'a> CaseResult<'a> {
  fn passed(&self) -> bool {
    self
      .cases
      .iter()
      .all(|c| matches!(c, CaseStatus::Validated | CaseStatus::Reported))
  }
}

#[derive(PartialEq, Eq, Debug)]
enum CaseStatus<'a> {
  /// Report no issue for valid code
  Validated,
  /// Report some issue for invalid code
  Reported,
  /// Report no issue for invalid code
  Missing(&'a str),
  /// Report some issue for valid code
  Noisy(&'a str),
}

fn report_case_number(output: &mut impl Write, test_cases: &[TestCase]) -> Result<()> {
  writeln!(output, "Running {} tests.", test_cases.len())?;
  Ok(())
}

trait Reporter {
  type Output: Write;
  fn get_output(&mut self) -> &mut Self::Output;
  /// A hook function runs before tests start.
  fn before_report(&mut self, test_cases: &[TestCase]) -> Result<()> {
    report_case_number(self.get_output(), test_cases)
  }
  /// A hook function runs after tests completed.
  fn after_report(&mut self, results: &[CaseResult]) -> Result<()> {
    let mut passed = 0;
    let mut failed = 0;
    for result in results {
      if result.passed() {
        passed += 1;
      } else {
        failed += 1;
      }
    }
    let result = if failed > 0 {
      Color::Red.paint("failed")
    } else {
      Color::Green.paint("ok")
    };
    writeln!(
      self.get_output(),
      "test result: {result}. {passed} passed; {failed} failed;"
    )?;
    Ok(())
  }

  fn report_failed_cases(&mut self, results: &[CaseResult]) -> Result<()> {
    let output = self.get_output();
    writeln!(output)?;
    writeln!(output, "----------- Failure Details -----------")?;
    for result in results {
      if result.passed() {
        continue;
      }
      for status in &result.cases {
        self.report_case_detail(result.id, status)?;
      }
    }
    Ok(())
  }

  fn report_case_summary(&mut self, case_id: &str, summary: &[CaseStatus]) -> Result<()>;
  fn report_case_detail(&mut self, case_id: &str, result: &CaseStatus) -> Result<()> {
    let output = self.get_output();
    let case_id = Style::new().bold().paint(case_id);
    let noisy = Style::new().underline().paint("Noisy");
    let missing = Style::new().underline().paint("Missing");
    match result {
      CaseStatus::Validated | CaseStatus::Reported => (),
      CaseStatus::Missing(s) => {
        writeln!(
          output,
          "[{missing}] Expect rule {case_id} to report issues, but none found in:"
        )?;
        writeln!(output, "```")?;
        writeln!(output, "{}", s)?;
        writeln!(output, "```")?;
      }
      CaseStatus::Noisy(s) => {
        writeln!(
          output,
          "[{noisy}] Expect {case_id} to report no issue, but some issues found in:"
        )?;
        writeln!(output, "```")?;
        writeln!(output, "{}", s)?;
        writeln!(output, "```")?;
      }
    }
    Ok(())
  }
}

struct DefaultReporter<Output: Write> {
  output: Output,
}

impl<O: Write> Reporter for DefaultReporter<O> {
  type Output = O;
  fn get_output(&mut self) -> &mut Self::Output {
    &mut self.output
  }
  fn report_case_summary(&mut self, case_id: &str, summary: &[CaseStatus]) -> Result<()> {
    let passed = summary
      .iter()
      .all(|c| matches!(c, CaseStatus::Validated | CaseStatus::Reported));
    let case_id = Style::new().bold().paint(case_id);
    let case_status = if summary.is_empty() {
      Color::Yellow.paint("SKIP")
    } else if passed {
      Color::Green.paint("PASS")
    } else {
      Color::Red.paint("FAIL")
    };
    let summary: String = summary
      .iter()
      .map(|s| match s {
        CaseStatus::Validated | CaseStatus::Reported => '.',
        CaseStatus::Missing(_) => 'M',
        CaseStatus::Noisy(_) => 'N',
      })
      .collect();
    writeln!(self.output, "{case_id}  {summary}  {case_status}")?;
    Ok(())
  }
}
// for result in summary {
//   match result {
//     CaseStatus::Validated => print!("✅"),
//     CaseStatus::Reported => print!("⛳"),
//     CaseStatus::Missing(_) => print!("❌"),
//     CaseStatus::Noisy(_) => print!("🚫"),
//   }
// }

// clippy does not allow submod with the same name with parent mod.
#[cfg(test)]
mod test_test {
  use super::*;
  use ast_grep_config::{PatternStyle, RuleConfig, SerializableRule, Severity};

  const TEST_RULE: &str = "test-rule";

  fn get_rule_config(rule: SerializableRule) -> RuleConfig<SupportLang> {
    RuleConfig {
      id: TEST_RULE.into(),
      message: "".into(),
      note: None,
      severity: Severity::Hint,
      language: SupportLang::TypeScript,
      rule,
      fix: None,
      constraints: None,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }
  fn always_report_rule() -> RuleCollection<SupportLang> {
    let rule = get_rule_config(SerializableRule::Pattern(PatternStyle::Str("$A".into())));
    RuleCollection::new(vec![rule])
  }
  fn never_report_rule() -> RuleCollection<SupportLang> {
    let rule = get_rule_config(SerializableRule::Not(Box::new(SerializableRule::Pattern(
      PatternStyle::Str("$A".into()),
    ))));
    RuleCollection::new(vec![rule])
  }

  fn valid_case() -> TestCase {
    TestCase {
      id: TEST_RULE.into(),
      valid: vec!["123".into()],
      invalid: vec![],
    }
  }

  fn invalid_case() -> TestCase {
    TestCase {
      id: TEST_RULE.into(),
      valid: vec![],
      invalid: vec!["123".into()],
    }
  }

  fn test_case_result(status: CaseStatus) -> Option<CaseResult> {
    Some(CaseResult {
      id: TEST_RULE,
      cases: vec![status],
    })
  }

  #[test]
  fn test_validated() {
    let rule = never_report_rule();
    let case = valid_case();
    let ret = verify_test_case_simple(&rule, &case);
    assert_eq!(ret, test_case_result(CaseStatus::Validated),);
  }

  #[test]
  fn test_reported() {
    let case = invalid_case();
    let rule = always_report_rule();
    let ret = verify_test_case_simple(&rule, &case);
    assert_eq!(ret, test_case_result(CaseStatus::Reported),);
  }
  #[test]
  fn test_noisy() {
    let case = valid_case();
    let rule = always_report_rule();
    let ret = verify_test_case_simple(&rule, &case);
    assert_eq!(ret, test_case_result(CaseStatus::Noisy("123")),);
  }
  #[test]
  fn test_missing() {
    let case = invalid_case();
    let rule = never_report_rule();
    let ret = verify_test_case_simple(&rule, &case);
    assert_eq!(ret, test_case_result(CaseStatus::Missing("123")),);
  }

  #[test]
  fn test_no_such_rule() {
    let case = TestCase {
      id: "no-such-rule".into(),
      valid: vec![],
      invalid: vec![],
    };
    let rule = never_report_rule();
    let ret = verify_test_case_simple(&rule, &case);
    assert!(ret.is_none());
  }
}
