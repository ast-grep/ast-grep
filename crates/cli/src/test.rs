use crate::config::find_config;
use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
enum LabelStyle {
  Primary,
  Secondary,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct Label {
  source: String,
  message: Option<String>,
  style: LabelStyle,
  start: usize,
  end: usize,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct TestSnapshot {
  pub id: String,
  pub source: String,
  pub fixed: Option<String>,
  pub labels: Vec<Label>,
}

#[derive(Args)]
pub struct TestArg {
  /// the directories to search test YAML files
  #[clap(short, long)]
  test_dir: PathBuf,
  #[clap(long)]
  snapshot_dir: Option<PathBuf>,
  /// Update the content of all snapshots that have changed in test.
  #[clap(short, long)]
  update_snapshots: bool,
  /// start an interactive session to update snapshots selectively
  #[clap(short, long)]
  interactive: bool,
}

pub fn run_test_rule(arg: TestArg) -> Result<()> {
  todo!()
}
