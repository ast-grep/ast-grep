use clap::Args;

use anyhow::Result;

/// TODO: add test arguments
#[derive(Args)]
pub struct TestArg {}

pub fn run_test_rule(_arg: TestArg) -> Result<()> {
  todo!("test sg rule is not implemented yet.")
}
