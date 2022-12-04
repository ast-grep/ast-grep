use crate::config::{find_config, find_tests, read_test_files};
use crate::error::ErrorContext;
use crate::interaction::{prompt, run_in_alternate_screen};
use crate::languages::{Language, SupportLang};
use ansi_term::{Color, Style};
use anyhow::{anyhow, Result};
use ast_grep_config::{RuleCollection, RuleConfig};
use ast_grep_core::{Node, NodeMatch};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Serialize, Deserialize)]
pub struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum LabelStyle {
  Primary,
  Secondary,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Label {
  source: String,
  message: Option<String>,
  style: LabelStyle,
  start: usize,
  end: usize,
}

impl Label {
  fn primary(n: &Node<SupportLang>) -> Self {
    let range = n.range();
    Self {
      source: n.text().to_string(),
      message: None,
      style: LabelStyle::Primary,
      start: range.start,
      end: range.end,
    }
  }

  fn secondary(n: &Node<SupportLang>) -> Self {
    let range = n.range();
    Self {
      source: n.text().to_string(),
      message: None,
      style: LabelStyle::Secondary,
      start: range.start,
      end: range.end,
    }
  }

  fn from_matched(n: NodeMatch<SupportLang>) -> Vec<Self> {
    let mut ret = vec![Self::primary(&n)];
    if let Some(secondary) = n.get_env().get_labels("secondary") {
      ret.extend(secondary.iter().map(Self::secondary));
    }
    ret
  }
}

use std::collections::HashMap;
type CaseId = String;
type Source = String;
#[derive(Serialize, Deserialize, Clone)]
pub struct TestSnapshots {
  pub id: CaseId,
  pub snapshots: HashMap<Source, TestSnapshot>,
}
pub type SnapshotCollection = HashMap<CaseId, TestSnapshots>;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct TestSnapshot {
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
  /// Only check if the test code is valid, without checking rule output.
  /// Turn it on when you want to ignore the output of rules.
  #[clap(long)]
  skip_snapshot_tests: bool,
  /// Update the content of all snapshots that have changed in test.
  #[clap(short, long)]
  update_snapshots: bool,
  /// start an interactive review to update snapshots selectively
  #[clap(short, long)]
  interactive: bool,
}

pub fn run_test_rule(arg: TestArg) -> Result<()> {
  if arg.interactive {
    let reporter = InteractiveReporter {
      output: std::io::stdout(),
      accepted_snapshots: HashMap::new(),
    };
    run_test_rule_impl(arg, reporter)
  } else {
    let reporter = DefaultReporter {
      output: std::io::stdout(),
      update_snapshots: arg.update_snapshots,
    };
    run_test_rule_impl(arg, reporter)
  }
}

fn parallel_collect<'a, T, R, F>(cases: &'a [T], filter_mapper: F) -> Vec<R>
where
  T: Sync,
  R: Send,
  F: FnMut(&'a T) -> Option<R> + Send + Copy,
{
  let cpu_count = num_cpus::get().min(12);
  let chunk_size = (cases.len() + cpu_count) / cpu_count;
  thread::scope(|s| {
    cases
      .chunks(chunk_size)
      .map(|chunk| {
        s.spawn(move || {
          chunk
            .iter()
            .filter_map(filter_mapper) // apply per case logic
            .collect::<Vec<_>>() // must collect here eagerly to consume iter in child threads
        })
      })
      .collect::<Vec<_>>() // must collect here eagerly to enable multi thread
      .into_iter()
      .flat_map(|sc| sc.join().unwrap())
      .collect()
  })
}

fn run_test_rule_impl<R: Reporter + Send>(arg: TestArg, reporter: R) -> Result<()> {
  let collections = &find_config(arg.config.clone())?;
  let (test_cases, snapshots) = if let Some(test_dir) = arg.test_dir {
    let base_dir = std::env::current_dir()?;
    let snapshot_dir = arg.snapshot_dir.as_deref();
    read_test_files(&base_dir, &test_dir, snapshot_dir)?
  } else {
    find_tests(arg.config)?
  };
  let snapshots = if arg.skip_snapshot_tests {
    None
  } else {
    Some(snapshots)
  };
  let reporter = &Arc::new(Mutex::new(reporter));
  {
    reporter.lock().unwrap().before_report(&test_cases)?;
  }

  let check_one_case = |case| {
    let result = verify_test_case_simple(collections, case, snapshots.as_ref());
    let mut reporter = reporter.lock().unwrap();
    if let Some(result) = result {
      reporter
        .report_case_summary(&case.id, &result.cases)
        .unwrap();
      Some(result)
    } else {
      let output = reporter.get_output();
      writeln!(output, "Configuraiont not found! {}", case.id).unwrap();
      None
    }
  };
  let results = parallel_collect(&test_cases, check_one_case);
  let mut reporter = reporter.lock().unwrap();
  let (passed, message) = reporter.after_report(&results)?;
  if passed {
    writeln!(reporter.get_output(), "{message}",)?;
    Ok(())
  } else {
    reporter.report_failed_cases(&results)?;
    Err(anyhow!(ErrorContext::TestFail(message)))
  }
}

enum SnapshotAction {
  /// Accept all changes
  AcceptAll,
  /// Reject all changes.
  AcceptNone,
  /// Delete outdated snapshots.
  Selectively(SnapshotCollection),
}

fn verify_invalid_case<'a>(
  rule_config: &RuleConfig<SupportLang>,
  case: &'a str,
  snapshot: Option<&TestSnapshots>,
) -> CaseStatus<'a> {
  let sg = rule_config.language.ast_grep(case);
  let rule = rule_config.get_rule();
  let Some(matched) = sg.root().find(&rule) else {
    return CaseStatus::Missing(case);
  };
  let labels = Label::from_matched(matched);
  let fixer = rule_config.get_fixer();
  let mut sg = sg;
  let fixed = if let Some(fix) = fixer {
    match sg.replace(rule, fix) {
      Ok(changed) => debug_assert!(changed),
      Err(_) => return CaseStatus::Error,
    };
    Some(sg.source().to_string())
  } else {
    None
  };
  let actual = TestSnapshot { fixed, labels };
  let Some(expected) = snapshot.and_then(|s| s.snapshots.get(case)) else {
    return CaseStatus::Wrong {
      source: case,
      actual,
      expected: None,
    }
  };
  if &actual == expected {
    CaseStatus::Reported
  } else {
    CaseStatus::Wrong {
      source: case,
      actual,
      expected: Some(expected.clone()),
    }
  }
}

fn verify_test_case_simple<'a>(
  rules: &RuleCollection<SupportLang>,
  test_case: &'a TestCase,
  snapshots: Option<&SnapshotCollection>,
) -> Option<CaseResult<'a>> {
  let rule_config = rules.get_rule(&test_case.id)?;
  let lang = rule_config.language;
  let rule = rule_config.get_rule();
  let valid_cases = test_case.valid.iter().map(|valid| {
    let sg = lang.ast_grep(valid);
    if sg.root().find(&rule).is_some() {
      CaseStatus::Noisy(valid)
    } else {
      CaseStatus::Validated
    }
  });
  let invalid_cases = test_case.invalid.iter();
  let cases = if let Some(snapshots) = snapshots {
    let snapshot = snapshots.get(&test_case.id);
    let invalid_cases =
      invalid_cases.map(|invalid| verify_invalid_case(rule_config, invalid, snapshot));
    valid_cases.chain(invalid_cases).collect()
  } else {
    let invalid_cases = invalid_cases.map(|invalid| {
      let sg = rule_config.language.ast_grep(invalid);
      let rule = rule_config.get_rule();
      if sg.root().find(rule).is_some() {
        CaseStatus::Reported
      } else {
        CaseStatus::Missing(invalid)
      }
    });
    valid_cases.chain(invalid_cases).collect()
  };
  Some(CaseResult {
    id: &test_case.id,
    cases,
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
  /// Reported no issue for valid code
  Validated,
  /// Reported correct issue for invalid code
  Reported,
  /// Reported wrong issues.
  Wrong {
    source: &'a str,
    actual: TestSnapshot,
    expected: Option<TestSnapshot>,
  },
  /// Reported no issue for invalid code
  Missing(&'a str),
  /// Reported some issue for valid code
  Noisy(&'a str),
  /// Error occurred when applying fix
  Error,
}

fn report_case_number(output: &mut impl Write, test_cases: &[TestCase]) -> Result<()> {
  writeln!(output, "Running {} tests", test_cases.len())?;
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
  fn after_report(&mut self, results: &[CaseResult]) -> Result<(bool, String)> {
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
      Ok((false, format!("test failed. {message}")))
    } else {
      let result = Color::Green.paint("ok");
      Ok((true, format!("test result: {result}. {message}")))
    }
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
        if !self.report_case_detail(result.id, status)? {
          break;
        }
      }
    }
    Ok(())
  }

  fn report_case_summary(&mut self, case_id: &str, summary: &[CaseStatus]) -> Result<()> {
    let passed = summary
      .iter()
      .all(|c| matches!(c, CaseStatus::Validated | CaseStatus::Reported));
    let style = Style::new().fg(Color::White).bold();
    let case_status = if summary.is_empty() {
      style.on(Color::Yellow).paint("SKIP")
    } else if passed {
      style.on(Color::Green).paint("PASS")
    } else {
      style.on(Color::Red).paint("FAIL")
    };
    let summary: String = summary
      .iter()
      .map(|s| match s {
        CaseStatus::Validated | CaseStatus::Reported => '.',
        CaseStatus::Wrong { .. } => 'W',
        CaseStatus::Missing(_) => 'M',
        CaseStatus::Noisy(_) => 'N',
        CaseStatus::Error => 'E',
      })
      .collect();
    writeln!(self.get_output(), "{case_status} {case_id}  {summary}")?;
    Ok(())
  }

  /// returns if should continue reporting
  fn report_case_detail(&mut self, case_id: &str, result: &CaseStatus) -> Result<bool> {
    report_case_detail_impl(self.get_output(), case_id, result)
  }
  fn collect_snapshot_action(&self) -> SnapshotAction;
}

fn report_case_detail_impl<W: Write>(
  output: &mut W,
  case_id: &str,
  result: &CaseStatus,
) -> Result<bool> {
  let case_id = Style::new().bold().paint(case_id);
  let noisy = Style::new().underline().paint("Noisy");
  let missing = Style::new().underline().paint("Missing");
  let wrong = Style::new().underline().paint("Wrong");
  let error = Style::new().underline().paint("Error");
  match result {
    CaseStatus::Validated | CaseStatus::Reported => (),
    CaseStatus::Wrong {
      source,
      actual,
      expected,
    } => {
      if let Some(expected) = expected {
        writeln!(
          output,
          "[{wrong}] {case_id} snapshot is different from baseline:"
        )?;
        if actual.fixed != expected.fixed {
          writeln!(output, "Fix:")?;
          writeln!(output, "Basline:\n{:?}", expected.fixed)?;
          writeln!(output, "Actual:\n{:?}", actual.fixed)?;
        }
        if actual.labels != expected.labels {
          writeln!(output, "Labels:")?;
          writeln!(output, "Basline:\n{:?}", expected.labels)?;
          writeln!(output, "Actual:\n{:?}", actual.labels)?;
        }
      } else {
        writeln!(output, "[{wrong}] No {case_id} basline found for code:")?;
        writeln!(output, "{}", source)?;
      }
    }
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
    CaseStatus::Error => {
      writeln!(output, "[{error}] Fail to apply fix to {case_id}")?;
    }
  }
  // continue
  Ok(true)
}

struct DefaultReporter<Output: Write> {
  output: Output,
  update_snapshots: bool,
}

impl<O: Write> Reporter for DefaultReporter<O> {
  type Output = O;

  fn get_output(&mut self) -> &mut Self::Output {
    &mut self.output
  }
  fn collect_snapshot_action(&self) -> SnapshotAction {
    if self.update_snapshots {
      SnapshotAction::AcceptAll
    } else {
      SnapshotAction::AcceptNone
    }
  }
}

struct InteractiveReporter<Output: Write> {
  output: Output,
  accepted_snapshots: SnapshotCollection,
}

const PROMPT: &str = "Accept new snapshot? (Yes[y], No[n], Accept All[a], Quit[q])";
impl<O: Write> Reporter for InteractiveReporter<O> {
  type Output = O;

  fn get_output(&mut self) -> &mut Self::Output {
    &mut self.output
  }

  fn collect_snapshot_action(&self) -> SnapshotAction {
    SnapshotAction::Selectively(self.accepted_snapshots.clone())
  }

  fn report_case_detail(&mut self, case_id: &str, result: &CaseStatus) -> Result<bool> {
    if matches!(result, CaseStatus::Validated | CaseStatus::Reported) {
      return Ok(true);
    }
    run_in_alternate_screen(|| {
      report_case_detail_impl(self.get_output(), case_id, result)?;
      if let CaseStatus::Wrong { source, actual, .. } = result {
        let response = prompt(PROMPT, "ynaq", Some('n'))?;
        match response {
          'y' => {
            if let Some(existing) = self.accepted_snapshots.get_mut(case_id) {
              existing
                .snapshots
                .insert(source.to_string(), actual.clone());
            } else {
              let mut snapshots = HashMap::new();
              snapshots.insert(source.to_string(), actual.clone());
              let shots = TestSnapshots {
                id: case_id.to_string(),
                snapshots,
              };
              self.accepted_snapshots.insert(case_id.to_string(), shots);
            }
            Ok(true)
          }
          'n' => Ok(true),
          'a' => todo!(),
          'q' => Ok(false),
          _ => unreachable!(),
        }
      } else {
        let response = prompt(PROMPT, "q", Some('\n'))?;
        Ok(response == 'q')
      }
    })
  }
}

// for result in summary {
//   match result {
//     CaseStatus::Validated => print!("✅"),
//     CaseStatus::Reported => print!("⛳"),
//     CaseStatus::Wrong(_) => print!("❌"),
//     CaseStatus::Missing(_) => print!("❌"),
//     CaseStatus::Noisy(_) => print!("🔊"),
//   }
// }

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_str, CompositeRule, RuleConfig, SerializableRule, Severity};

  const TEST_RULE: &str = "test-rule";

  fn get_rule_config(rule: SerializableRule) -> RuleConfig<SupportLang> {
    RuleConfig {
      id: TEST_RULE.into(),
      message: "test".into(),
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
    // empty all should mean always
    let serialized = SerializableRule::Composite(CompositeRule::All(vec![]));
    let rule = get_rule_config(serialized);
    RuleCollection::try_new(vec![rule]).expect("RuleCollection must be valid")
  }
  fn never_report_rule() -> RuleCollection<SupportLang> {
    // empty any should mean never
    let serialized = SerializableRule::Composite(CompositeRule::Any(vec![]));
    let rule = get_rule_config(serialized);
    RuleCollection::try_new(vec![rule]).expect("RuleCollection must be valid")
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
    let ret = verify_test_case_simple(&rule, &case, None);
    assert_eq!(ret, test_case_result(CaseStatus::Validated),);
  }

  #[test]
  fn test_reported() {
    let case = invalid_case();
    let rule = always_report_rule();
    let ret = verify_test_case_simple(&rule, &case, None);
    assert_eq!(ret, test_case_result(CaseStatus::Reported),);
  }
  #[test]
  fn test_noisy() {
    let case = valid_case();
    let rule = always_report_rule();
    let ret = verify_test_case_simple(&rule, &case, None);
    assert_eq!(ret, test_case_result(CaseStatus::Noisy("123")),);
  }
  #[test]
  fn test_missing() {
    let case = invalid_case();
    let rule = never_report_rule();
    let ret = verify_test_case_simple(&rule, &case, None);
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
    let ret = verify_test_case_simple(&rule, &case, None);
    assert!(ret.is_none());
  }

  #[test]
  fn test_snapshot() {
    let serialize = from_str("pattern: let a = 1").expect("should parse");
    let rule = get_rule_config(serialize);
    let ret = verify_invalid_case(&rule, "function () { let a = 1 }", None);
    assert!(matches!(&ret, CaseStatus::Wrong { expected: None, .. }));
    let CaseStatus::Wrong { actual, source, .. } = ret else {
        panic!("wrong");
    };
    assert_eq!(source, "function () { let a = 1 }");
    let primary = &actual.labels[0];
    assert_eq!(primary.source, "let a = 1");
    let mut snapshots = HashMap::new();
    snapshots.insert(source.to_string(), actual);
    let test_snapshots = TestSnapshots {
      id: TEST_RULE.to_string(),
      snapshots,
    };
    let ret = verify_invalid_case(&rule, "function () { let a = 1 }", Some(&test_snapshots));
    assert!(matches!(ret, CaseStatus::Reported));
  }
}
