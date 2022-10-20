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
  let mut test_pass = true;
  for test_case in test_cases {
    test_pass = verify_test_case_simple(&collections, test_case) && test_pass;
  }
  if test_pass {
    println!("All tests passed");
    Ok(())
  } else {
    Err(anyhow::anyhow!("Some tests failed"))
  }
}

fn verify_test_case_simple(collections: &RuleCollection<SupportLang>, test_case: TestCase) -> bool {
  let mut test_pass = true;
  let rule = match collections.get_rule(&test_case.id) {
    Some(r) => r,
    None => {
      println!("Configuraiont not found! {}", test_case.id);
      return false;
    }
  };
  let lang = rule.language;
  let rule = rule.get_rule();
  let bold = Style::new().bold();
  for valid in test_case.valid {
    let sg = lang.ast_grep(&valid);
    if sg.root().find(&rule).is_some() {
      println!(
        "{} ... {}: finds issue(s) for valid code.",
        bold.paint(&test_case.id),
        Color::Red.paint("FAIL")
      );
      test_pass = false;
    } else {
    }
  }
  for invalid in test_case.invalid {
    let sg = lang.ast_grep(&invalid);
    if sg.root().find(&rule).is_none() {
      println!(
        "{} ... {}: reports no issue for invalid code.",
        bold.paint(&test_case.id),
        Color::Red.paint("FAIL")
      );
      test_pass = false;
    }
  }
  if test_pass {
    println!(
      "{} ... {}",
      bold.paint(&test_case.id),
      Color::Green.paint("PASS")
    );
  }
  test_pass
}
