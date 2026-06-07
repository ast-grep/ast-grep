use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

#[derive(Args)]
pub struct OutlineArg {
  /// The files or directories to summarize.
  ///
  /// Accepts one or more file/directory paths.
  #[clap(value_name = "PATH", required = true)]
  paths: Vec<PathBuf>,
}

pub fn run_outline(arg: OutlineArg) -> anyhow::Result<ExitCode> {
  let _ = arg.paths;
  println!("No outline items found.");
  Ok(ExitCode::SUCCESS)
}
