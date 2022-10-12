use clap::Args;
use serde::{Serialize, Deserialize};
use anyhow::Result;

#[derive(Serialize)]
struct TestCase {
  pub id: String,
  #[serde(default)]
  pub valid: Vec<String>,
  #[serde(default)]
  pub invalid: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Issue {
  message: String,
  start: usize,
  end: usize,
}

#[derive(Serialize, Deserialize)]
struct TestSnapshot {
  pub id: String,
  pub source: String,
  pub fixed: Option<String>,
  pub issues: Vec<Issue>,
}

/// TODO: add test arguments
#[derive(Args)]
pub struct TestArg {}

pub fn run_test_rule(_arg: TestArg) -> Result<()> {
  todo!("test sg rule is not implemented yet.")
}
