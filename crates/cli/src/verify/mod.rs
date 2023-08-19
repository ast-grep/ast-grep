mod case_result;
mod find_file;
mod reporter;
mod snapshot;

use crate::config::{find_rules, register_custom_language};
use crate::error::ErrorContext;
use crate::lang::SgLang;
use anyhow::{anyhow, Result};
use ast_grep_config::{RuleCollection, RuleConfig};
use ast_grep_core::{Node as SgNode, StrDoc};
use ast_grep_language::Language;
use clap::Args;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::to_string;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

pub use case_result::{CaseResult, CaseStatus, SnapshotAction};
use find_file::{find_tests, read_test_files, TestHarness};
use reporter::{DefaultReporter, InteractiveReporter, Reporter};
use snapshot::TestSnapshot;
pub use snapshot::{SnapshotCollection, TestSnapshots};

type Node<'a, L> = SgNode<'a, StrDoc<L>>;

/// Corresponds to one rule-test.yml for testing.
///
/// A rule-test contains these fields:
/// * id: the id of the rule that will be tested against
/// * valid: code that we do not expect to have any issues
/// * invalid: code that we do expect to have some issues
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
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
  let collections = &find_rules(arg.config.clone(), None)?;
  let TestHarness {
    test_cases,
    snapshots,
    path_map,
  } = if let Some(test_dirname) = arg.test_dir {
    let base_dir = std::env::current_dir()?;
    let snapshot_dirname = arg.snapshot_dir.as_deref();
    read_test_files(
      &base_dir,
      &test_dirname,
      snapshot_dirname,
      arg.filter.as_ref(),
    )?
  } else {
    find_tests(arg.config, arg.filter.as_ref())?
  };
  let snapshots = (!arg.skip_snapshot_tests).then_some(snapshots);
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

fn verify_invalid_case<'a>(
  rule_config: &RuleConfig<SgLang>,
  case: &'a str,
  snapshot: Option<&TestSnapshots>,
) -> CaseStatus<'a> {
  let actual = match TestSnapshot::generate(rule_config, case) {
    Ok(Some(snapshot)) => snapshot,
    Ok(None) => return CaseStatus::Missing(case),
    Err(_) => return CaseStatus::Error,
  };
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

// for result in summary {
//   match result {
//     CaseStatus::Validated => print!("✅"),
//     CaseStatus::Reported => print!("⛳"),
//     CaseStatus::Wrong(_) => print!("❌"),
//     CaseStatus::Missing(_) => print!("❌"),
//     CaseStatus::Noisy(_) => print!("🔊"),
//   }
// }

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
  /// Start an interactive review to update snapshots selectively
  #[clap(short, long)]
  interactive: bool,
  /// Only run rule test cases that matches REGEX.
  #[clap(short, long, value_name = "REGEX")]
  filter: Option<Regex>,
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

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_str, GlobalRules, RuleConfig};

  const TEST_RULE: &str = "test-rule";

  fn get_rule_text(rule: &str) -> String {
    format!(
      "
id: {TEST_RULE}
message: test
severity: hint
language: TypeScript
rule:
  {rule}
"
    )
  }

  fn get_rule_config(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    let inner = from_str(&get_rule_text(rule)).unwrap();
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
  fn test_run_verify_error() {
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
      filter: None,
    };
    assert!(run_test_rule_impl(arg, reporter).is_err());
  }
  const TRANSFORM_TEXT: &str = "
transform:
  B:
    substring:
      source: $A
      startChar: 1
      endChar: -1
fix: 'log($B)'";
  #[test]
  fn test_verify_transform() {
    let globals = GlobalRules::default();
    let inner = from_str(&get_rule_text(&format!(
      "pattern: console.log($A)\n{}",
      TRANSFORM_TEXT
    )))
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
