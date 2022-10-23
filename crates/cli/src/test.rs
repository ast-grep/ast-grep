use crate::config::{find_config, find_tests, read_test_files};
use crate::languages::{Language, SupportLang};
use ansi_term::{Color, Style};
use anyhow::Result;
use ast_grep_config::RuleCollection;
use clap::Args;
use serde::{Deserialize, Serialize};
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
  let collections = find_config(arg.config.clone())?;
  let (test_cases, _snapshots) = if let Some(test_dir) = arg.test_dir {
    let base_dir = std::env::current_dir()?;
    let snapshot_dir = arg.snapshot_dir.as_deref();
    read_test_files(&base_dir, &test_dir, snapshot_dir)?
  } else {
    find_tests(arg.config)?
  };
  let reporter = DefaultReporter;
  reporter.before_report(&test_cases);
  let results: Vec<_> = test_cases
    .iter()
    .map(|case| {
      let result = verify_test_case_simple(&collections, case);
      reporter.report_case_summary(&case.id, &result.cases);
      result
    })
    .collect();
  reporter.report_failed_cases(&results);
  reporter.after_report(&results);
  Ok(())
}

fn verify_test_case_simple<'a>(
  rules: &RuleCollection<SupportLang>,
  test_case: &'a TestCase,
) -> CaseResult<'a> {
  let rule = match rules.get_rule(&test_case.id) {
    Some(r) => r,
    None => {
      println!("Configuraiont not found! {}", test_case.id);
      return CaseResult::default();
    }
  };
  let lang = rule.language;
  let rule = rule.get_rule();
  let valid_cases = test_case.valid.iter().map(|valid| {
    let sg = lang.ast_grep(valid);
    if sg.root().find(&rule).is_some() {
      CaseStatus::FalseAlarm(valid)
    } else {
      CaseStatus::Hit
    }
  });
  let invalid_cases = test_case.invalid.iter().map(|invalid| {
    let sg = lang.ast_grep(invalid);
    if sg.root().find(&rule).is_none() {
      CaseStatus::Miss(invalid)
    } else {
      CaseStatus::CorrectReject
    }
  });
  CaseResult {
    id: &test_case.id,
    cases: valid_cases.chain(invalid_cases).collect(),
  }
}

#[derive(Default)]
struct CaseResult<'a> {
  id: &'a str,
  cases: Vec<CaseStatus<'a>>,
}

impl<'a> CaseResult<'a> {
  fn passed(&self) -> bool {
    self
      .cases
      .iter()
      .all(|c| matches!(c, CaseStatus::Hit | CaseStatus::CorrectReject))
  }
}

enum CaseStatus<'a> {
  Hit,
  CorrectReject,
  Miss(&'a str),
  FalseAlarm(&'a str),
}

fn report_case_number(test_cases: &[TestCase]) {
  println!("Running {} tests.", test_cases.len());
}

trait Reporter {
  /// A hook function runs before tests start.
  fn before_report(&self, test_cases: &[TestCase]) {
    report_case_number(test_cases);
  }
  /// A hook function runs after tests completed.
  fn after_report(&self, results: &[CaseResult]) {
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
    println!("test result: {result}. {passed} passed; {failed} failed;");
  }

  fn report_failed_cases(&self, results: &[CaseResult]) {
    println!();
    println!("----------- Failure Details -----------");
    for result in results {
      if result.passed() {
        continue;
      }
      for status in &result.cases {
        self.report_case_detail(result.id, status);
      }
    }
  }

  fn report_case_summary(&self, case_id: &str, summary: &[CaseStatus]);
  fn report_case_detail(&self, case_id: &str, result: &CaseStatus) {
    let case_id = Style::new().bold().paint(case_id);
    let false_alarm = Style::new().underline().paint("False Alarm");
    let miss_report = Style::new().underline().paint("Miss Report");
    match result {
      CaseStatus::Hit | CaseStatus::CorrectReject => (),
      CaseStatus::Miss(s) => {
        println!("[{miss_report}] Expect rule {case_id} to report issues, but none found in:");
        println!("```");
        println!("{}", s);
        println!("```");
      }
      CaseStatus::FalseAlarm(s) => {
        println!("[{false_alarm}] Expect {case_id} to report no issue, but some issues found in:");
        println!("```");
        println!("{}", s);
        println!("```");
      }
    }
  }
}

struct DefaultReporter;

impl Reporter for DefaultReporter {
  fn report_case_summary(&self, case_id: &str, summary: &[CaseStatus]) {
    let passed = summary
      .iter()
      .all(|c| matches!(c, CaseStatus::Hit | CaseStatus::CorrectReject));
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
        CaseStatus::Hit | CaseStatus::CorrectReject => '.',
        CaseStatus::Miss(_) => 'M',
        CaseStatus::FalseAlarm(_) => 'F',
      })
      .collect();
    println!("{case_id}  {summary}  {case_status}");
  }
}
// for result in summary {
//   match result {
//     CaseStatus::Hit => print!("✅"),
//     CaseStatus::CorrectReject => print!("⛳"),
//     CaseStatus::Miss(_) => print!("❌"),
//     CaseStatus::FalseAlarm(_) => print!("🚫"),
//   }
// }
