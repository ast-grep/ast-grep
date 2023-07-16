use crate::config::{
  find_rules, find_tests, read_test_files, register_custom_language, TestHarness,
};
use crate::error::ErrorContext;
use crate::lang::SgLang;
use crate::print::{print_diff, ColorChoice, PrintStyles};
use crate::utils::{prompt, run_in_alternate_screen};
use ansi_term::{Color, Style};
use anyhow::{anyhow, Result};
use ast_grep_config::{RuleCollection, RuleConfig};
use ast_grep_core::{Node as SgNode, NodeMatch, StrDoc};
use ast_grep_language::Language;
use clap::Args;
use serde::{Deserialize, Serialize, Serializer};
use serde_yaml::to_string;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

type Node<'a, L> = SgNode<'a, StrDoc<L>>;

fn ordered_map<S>(value: &HashMap<String, TestSnapshot>, serializer: S) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  let ordered: BTreeMap<_, _> = value.iter().collect();
  ordered.serialize(serializer)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum LabelStyle {
  Primary,
  Secondary,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Label {
  source: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  message: Option<String>,
  style: LabelStyle,
  start: usize,
  end: usize,
}

impl Label {
  fn primary(n: &Node<SgLang>) -> Self {
    let range = n.range();
    Self {
      source: n.text().to_string(),
      message: None,
      style: LabelStyle::Primary,
      start: range.start,
      end: range.end,
    }
  }

  fn secondary(n: &Node<SgLang>) -> Self {
    let range = n.range();
    Self {
      source: n.text().to_string(),
      message: None,
      style: LabelStyle::Secondary,
      start: range.start,
      end: range.end,
    }
  }

  fn from_matched(n: NodeMatch<StrDoc<SgLang>>) -> Vec<Self> {
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
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestSnapshots {
  pub id: CaseId,
  #[serde(serialize_with = "ordered_map")]
  pub snapshots: HashMap<Source, TestSnapshot>,
}

pub type SnapshotCollection = HashMap<CaseId, TestSnapshots>;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestSnapshot {
  #[serde(skip_serializing_if = "Option::is_none")]
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
  /// Conflicts with --update-all.
  #[clap(long, conflicts_with = "update_all")]
  skip_snapshot_tests: bool,
  /// Update the content of all snapshots that have changed in test.
  /// Conflicts with --skip-snapshot-tests.
  #[clap(short = 'U', long)]
  update_all: bool,
  /// start an interactive review to update snapshots selectively
  #[clap(short, long)]
  interactive: bool,
}

pub fn run_test_rule(arg: TestArg) -> Result<()> {
  register_custom_language(arg.config.clone());
  if arg.interactive {
    let reporter = InteractiveReporter {
      output: std::io::stdout(),
      accepted_snapshots: HashMap::new(),
      should_accept_all: false,
    };
    run_test_rule_impl(arg, reporter)
  } else {
    let reporter = DefaultReporter {
      output: std::io::stdout(),
      update_all: arg.update_all,
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
  let collections = &find_rules(arg.config.clone())?;
  let TestHarness {
    test_cases,
    snapshots,
    path_map,
  } = if let Some(test_dir) = arg.test_dir {
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
      writeln!(output, "Configuration not found! {}", case.id).unwrap();
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
    let action = reporter.collect_snapshot_action();
    apply_snapshot_action(action, &results, snapshots, path_map)?;
    Err(anyhow!(ErrorContext::TestFail(message)))
  }
}

fn apply_snapshot_action(
  action: SnapshotAction,
  results: &[CaseResult],
  snapshots: Option<SnapshotCollection>,
  path_map: HashMap<String, PathBuf>,
) -> Result<()> {
  let Some(snapshots) = snapshots else {
    return Ok(())
  };
  let accepted = match action {
    SnapshotAction::AcceptAll => {
      let mut snapshot_collection = HashMap::new();
      for result in results {
        let case_id = result.id.to_string();
        snapshot_collection.insert(case_id.clone(), result.changed_snapshots());
      }
      snapshot_collection
    }
    SnapshotAction::AcceptNone => return Ok(()),
    SnapshotAction::Selectively(a) => a,
  };
  let merged = merge_snapshots(accepted, snapshots);
  write_merged_to_disk(merged, path_map)
}
fn merge_snapshots(
  accepted: SnapshotCollection,
  mut old: SnapshotCollection,
) -> SnapshotCollection {
  for (id, tests) in accepted {
    if let Some(existing) = old.get_mut(&id) {
      existing.snapshots.extend(tests.snapshots);
    } else {
      old.insert(id, tests);
    }
  }
  old
}
fn write_merged_to_disk(
  merged: SnapshotCollection,
  path_map: HashMap<String, PathBuf>,
) -> Result<()> {
  for (id, snaps) in merged {
    // TODO
    let path = path_map.get(&id).unwrap();
    if !path.exists() {
      std::fs::create_dir(path)?;
    }
    let file = path.join(format!("{id}-snapshot.yml"));
    std::fs::write(file, to_string(&snaps)?)?;
  }
  Ok(())
}

#[derive(Debug)]
enum SnapshotAction {
  /// Accept all changes
  AcceptAll,
  /// Reject all changes.
  AcceptNone,
  /// Delete outdated snapshots.
  Selectively(SnapshotCollection),
}

fn verify_invalid_case<'a>(
  rule_config: &RuleConfig<SgLang>,
  case: &'a str,
  snapshot: Option<&TestSnapshots>,
) -> CaseStatus<'a> {
  let sg = rule_config.language.ast_grep(case);
  let rule = &rule_config.matcher;
  let Some(matched) = sg.root().find(rule) else {
    return CaseStatus::Missing(case);
  };
  let labels = Label::from_matched(matched);
  let fixer = &rule_config.fixer;
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
  rules: &RuleCollection<SgLang>,
  test_case: &'a TestCase,
  snapshots: Option<&SnapshotCollection>,
) -> Option<CaseResult<'a>> {
  let rule_config = rules.get_rule(&test_case.id)?;
  let lang = rule_config.language;
  let rule = &rule_config.matcher;
  let valid_cases = test_case.valid.iter().map(|valid| {
    let sg = lang.ast_grep(valid);
    if sg.root().find(rule).is_some() {
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
      let rule = &rule_config.matcher;
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
  fn changed_snapshots(&self) -> TestSnapshots {
    let snapshots = self
      .cases
      .iter()
      .filter_map(|c| match c {
        CaseStatus::Wrong { source, actual, .. } => Some((source.to_string(), actual.clone())),
        _ => None,
      })
      .collect();
    TestSnapshots {
      id: self.id.to_string(),
      snapshots,
    }
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
          return Ok(());
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
    let summary = report_summary(summary);
    writeln!(self.get_output(), "{case_status} {case_id}  {summary}")?;
    Ok(())
  }

  /// returns if should continue reporting
  fn report_case_detail(&mut self, case_id: &str, result: &CaseStatus) -> Result<bool> {
    report_case_detail_impl(self.get_output(), case_id, result)
  }
  fn collect_snapshot_action(&self) -> SnapshotAction;
}

fn report_summary(summary: &[CaseStatus]) -> String {
  if summary.len() > 40 {
    let mut pass = 0;
    let mut wrong = 0;
    let mut missing = 0;
    let mut noisy = 0;
    let mut error = 0;
    for s in summary {
      match s {
        CaseStatus::Validated | CaseStatus::Reported => pass += 1,
        CaseStatus::Wrong { .. } => wrong += 1,
        CaseStatus::Missing(_) => missing += 1,
        CaseStatus::Noisy(_) => noisy += 1,
        CaseStatus::Error => error += 1,
      }
    }
    let stats = vec![
      ("Pass", pass),
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
) -> Result<bool> {
  let case_id = Style::new().bold().paint(case_id);
  let noisy = Style::new().underline().paint("Noisy");
  let missing = Style::new().underline().paint("Missing");
  let wrong = Style::new().underline().paint("Wrong");
  let error = Style::new().underline().paint("Error");
  let styles = PrintStyles::from(ColorChoice::Auto);
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
          "[{wrong}] {case_id} snapshot is different from baseline."
        )?;
        let actual_str = to_string(&actual)?;
        let expected_str = to_string(&expected)?;
        writeln!(output, "{}", Style::new().italic().paint("Diff:"))?;
        print_diff(&expected_str, &actual_str, &styles, output)?;
      } else {
        writeln!(output, "[{wrong}] No {case_id} basline found.")?;
        // TODO: add to print_styles
        writeln!(
          output,
          "{}",
          Style::new().italic().paint("Generated Snapshot:")
        )?;
        indented_write(output, &to_string(&actual)?)?;
      }
      // TODO: add to print_styles
      writeln!(output, "{}", Style::new().italic().paint("For Code:"))?;
      indented_write(output, source)?;
      writeln!(output)?;
    }
    CaseStatus::Missing(s) => {
      writeln!(
        output,
        "[{missing}] Expect rule {case_id} to report issues, but none found in:"
      )?;
      writeln!(output)?;
      indented_write(output, s)?;
      writeln!(output)?;
    }
    CaseStatus::Noisy(s) => {
      writeln!(
        output,
        "[{noisy}] Expect {case_id} to report no issue, but some issues found in:"
      )?;
      writeln!(output)?;
      indented_write(output, s)?;
      writeln!(output)?;
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
  update_all: bool,
}

impl<O: Write> Reporter for DefaultReporter<O> {
  type Output = O;

  fn get_output(&mut self) -> &mut Self::Output {
    &mut self.output
  }
  fn collect_snapshot_action(&self) -> SnapshotAction {
    if self.update_all {
      SnapshotAction::AcceptAll
    } else {
      SnapshotAction::AcceptNone
    }
  }
}

struct InteractiveReporter<Output: Write> {
  output: Output,
  accepted_snapshots: SnapshotCollection,
  should_accept_all: bool,
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
        let mut accept = || {
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
        };
        if self.should_accept_all {
          return accept();
        }
        let response = prompt(PROMPT, "ynaq", Some('n'))?;
        match response {
          'y' => accept(),
          'n' => Ok(true),
          'a' => {
            self.should_accept_all = true;
            accept()
          }
          'q' => Ok(false),
          _ => unreachable!(),
        }
      } else {
        let response = prompt("Next[enter], Quit[q]", "q", Some('\n'))?;
        Ok(response != 'q')
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
  use ast_grep_config::{from_str, GlobalRules, RuleConfig};

  const TEST_RULE: &str = "test-rule";

  fn get_rule_config(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    let inner = from_str(&format!(
      "
id: {TEST_RULE}
message: test
severity: hint
language: TypeScript
rule:
  {rule}
"
    ))
    .unwrap();
    RuleConfig::try_from(inner, &globals).unwrap()
  }
  fn always_report_rule() -> RuleCollection<SgLang> {
    // empty all should mean always
    let rule = get_rule_config("all: []");
    RuleCollection::try_new(vec![rule]).expect("RuleCollection must be valid")
  }
  fn never_report_rule() -> RuleCollection<SgLang> {
    // empty any should mean never
    let rule = get_rule_config("any: []");
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
    let rule = get_rule_config("pattern: let a = 1");
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

  use codespan_reporting::term::termcolor::Buffer;
  #[test]
  fn test_run_verify() {
    let reporter = DefaultReporter {
      output: Buffer::no_color(),
      update_all: false,
    };
    let arg = TestArg {
      config: None,
      interactive: false,
      skip_snapshot_tests: true,
      snapshot_dir: None,
      test_dir: None,
      update_all: false,
    };
    // TODO
    assert!(run_test_rule_impl(arg, reporter).is_err());
  }

  #[test]
  fn test_verify_transform() {
    let globals = GlobalRules::default();
    let inner = from_str(
      "
id: test-rule
message: test
severity: hint
language: TypeScript
rule:
  pattern: console.log($A)
transform:
  B:
    substring:
      source: $A
      startChar: 1
      endChar: -1
fix: 'log($B)'
",
    )
    .unwrap();
    let rule = RuleConfig::try_from(inner, &globals).unwrap();
    let rule = RuleCollection::try_new(vec![rule]).expect("RuleCollection must be valid");
    let case = TestCase {
      id: TEST_RULE.into(),
      valid: vec![],
      invalid: vec!["console.log(123)".to_string()],
    };
    let snapshots = SnapshotCollection::new();
    let mut ret = verify_test_case_simple(&rule, &case, Some(&snapshots)).unwrap();
    let case = ret.cases.pop().unwrap();
    match case {
      CaseStatus::Wrong { actual, .. } => {
        assert_eq!(actual.fixed.unwrap(), "log(2)");
      }
      _ => {
        panic!("wrong case status");
      }
    }
  }
}
