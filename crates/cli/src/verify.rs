mod case_result;
mod find_file;
mod reporter;
mod snapshot;
mod test_case;

use crate::config::ProjectConfig;
use crate::lang::SgLang;
use crate::utils::ErrorContext;
use anyhow::{anyhow, Result};
use ast_grep_config::RuleCollection;
use ast_grep_core::{Node as SgNode, StrDoc};
use clap::Args;
use regex::Regex;
use serde_yaml::to_string;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use case_result::{CaseResult, CaseStatus};
use find_file::TestHarness;
use reporter::{DefaultReporter, InteractiveReporter, Reporter};
use snapshot::{SnapshotAction, SnapshotCollection, TestSnapshots};
use test_case::TestCase;

type Node<'a, L> = SgNode<'a, StrDoc<L>>;

fn parallel_collect<'a, T, R, F>(cases: &'a [T], filter_mapper: F) -> Vec<R>
where
  T: Sync,
  R: Send,
  F: FnMut(&'a T) -> Option<R> + Send + Copy,
{
  let threads = std::thread::available_parallelism()
    .map_or(1, |n| n.get())
    .min(12);
  let chunk_size = (cases.len() + threads) / threads;
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

fn run_test_rule_impl<R: Reporter + Send>(
  arg: TestArg,
  reporter: R,
  project: ProjectConfig,
) -> Result<()> {
  let collections = &project.find_rules(Default::default())?.0;
  let TestHarness {
    test_cases,
    snapshots,
    path_map,
  } = if let Some(test_dirname) = arg.test_dir {
    let snapshot_dirname = arg.snapshot_dir.as_deref();
    TestHarness::from_dir(&test_dirname, snapshot_dirname, arg.filter.as_ref())?
  } else {
    TestHarness::from_config(project, arg.filter.as_ref())?
  };
  let snapshots = (!arg.skip_snapshot_tests).then_some(snapshots);
  let reporter = &Arc::new(Mutex::new(reporter));
  {
    reporter.lock().unwrap().before_report(&test_cases)?;
  }

  let check_one_case = |case| {
    let result = verify_test_case_simple(case, collections, snapshots.as_ref());
    if result.is_none() {
      let mut reporter = reporter.lock().unwrap();
      let output = reporter.get_output();
      writeln!(output, "Configuration not found! {}", case.id).unwrap();
    }
    result
  };
  let mut results = parallel_collect(&test_cases, check_one_case);
  let mut reporter = reporter.lock().unwrap();

  reporter.report_failed_cases(&mut results)?;
  let action = reporter.collect_snapshot_action();
  apply_snapshot_action(action, &results, snapshots, path_map)?;
  reporter.report_summaries(&results)?;
  let (passed, message) = reporter.after_report(&results)?;
  if passed {
    writeln!(reporter.get_output(), "{message}",)?;
    Ok(())
  } else {
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
    return Ok(());
  };
  let Some(merged) = action.update_snapshot_collection(snapshots, results) else {
    return Ok(());
  };
  write_merged_to_disk(merged, path_map)
}

fn write_merged_to_disk(
  merged: SnapshotCollection,
  path_map: HashMap<String, PathBuf>,
) -> Result<()> {
  for (id, snaps) in merged {
    let path = &path_map[&id];
    if !path.exists() {
      std::fs::create_dir(path)?;
    }
    let file = path.join(format!("{id}-snapshot.yml"));
    std::fs::write(file, to_string(&snaps)?)?;
  }
  Ok(())
}

fn verify_test_case_simple<'a>(
  test_case: &'a TestCase,
  rules: &RuleCollection<SgLang>,
  snapshots: Option<&SnapshotCollection>,
) -> Option<CaseResult<'a>> {
  let rule_config = rules.get_rule(&test_case.id)?;
  let test_case = if let Some(snapshots) = snapshots {
    let snaps = snapshots.get(&test_case.id);
    test_case.verify_with_snapshot(rule_config, snaps)
  } else {
    test_case.verify_rule(rule_config)
  };
  Some(test_case)
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

pub fn run_test_rule(arg: TestArg, project: Result<ProjectConfig>) -> Result<()> {
  let project = project?;
  if arg.interactive {
    let reporter = InteractiveReporter {
      output: std::io::stdout(),
      should_accept_all: false,
    };
    run_test_rule_impl(arg, reporter, project)
  } else {
    let reporter = DefaultReporter {
      output: std::io::stdout(),
      update_all: arg.update_all,
    };
    run_test_rule_impl(arg, reporter, project)
  }
}

#[cfg(test)]
pub mod test {
  use super::*;
  use ast_grep_config::{from_str, GlobalRules, RuleConfig};

  pub const TEST_RULE: &str = "test-rule";

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

  pub fn get_rule_config(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    let inner = from_str(&get_rule_text(rule)).unwrap();
    RuleConfig::try_from(inner, &globals).unwrap()
  }
  fn always_report_rule() -> RuleCollection<SgLang> {
    // empty all should mean always
    let rule = get_rule_config("all: [kind: number]");
    RuleCollection::try_new(vec![rule]).expect("RuleCollection must be valid")
  }
  fn never_report_rule() -> RuleCollection<SgLang> {
    // empty any should mean never
    let rule = get_rule_config("any: [kind: string]");
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
    let ret = verify_test_case_simple(&case, &rule, None);
    assert_eq!(ret, test_case_result(CaseStatus::Validated),);
  }

  #[test]
  fn test_reported() {
    let case = invalid_case();
    let rule = always_report_rule();
    let ret = verify_test_case_simple(&case, &rule, None);
    assert_eq!(ret, test_case_result(CaseStatus::Reported),);
  }
  #[test]
  fn test_noisy() {
    let case = valid_case();
    let rule = always_report_rule();
    let ret = verify_test_case_simple(&case, &rule, None);
    assert_eq!(ret, test_case_result(CaseStatus::Noisy("123")),);
  }
  #[test]
  fn test_missing() {
    let case = invalid_case();
    let rule = never_report_rule();
    let ret = verify_test_case_simple(&case, &rule, None);
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
    let ret = verify_test_case_simple(&case, &rule, None);
    assert!(ret.is_none());
  }

  #[test]
  fn test_run_verify_error() {
    let arg = TestArg {
      interactive: false,
      skip_snapshot_tests: true,
      snapshot_dir: None,
      test_dir: None,
      update_all: false,
      filter: None,
    };
    assert!(run_test_rule(arg, Err(anyhow!("error"))).is_err());
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
    let mut ret = verify_test_case_simple(&case, &rule, Some(&snapshots)).unwrap();
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
