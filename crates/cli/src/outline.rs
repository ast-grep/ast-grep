mod default_rule;

use std::process::ExitCode;

use clap::Args;

#[derive(Args)]
pub struct OutlineArg {}

pub fn run_outline(_: OutlineArg) -> anyhow::Result<ExitCode> {
  todo!()
}
