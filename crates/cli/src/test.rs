use crate::config::{find_config, find_tests, read_test_files};
use crate::languages::{Language, SupportLang};
use anyhow::Result;
use ast_grep_config::RuleCollection;
use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
  #[clap(long)]
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
  for test_case in test_cases {
    verify_test_case_simple(&collections, test_case);
  }
  Ok(())
}

fn verify_test_case_simple(collections: &RuleCollection<SupportLang>, test_case: TestCase) {
  let rule = match collections.get_rule(&test_case.id) {
    Some(r) => r,
    None => {
      eprintln!("Configuraiont not found! {}", test_case.id);
      return;
    }
  };
  let lang = rule.language;
  let rule = rule.get_rule();
  for valid in test_case.valid {
    let sg = lang.ast_grep(&valid);
    assert!(sg.root().find(&rule).is_none());
  }
  for invalid in test_case.invalid {
    let sg = lang.ast_grep(&invalid);
    assert!(sg.root().find(&rule).is_some());
  }
}
