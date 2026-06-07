// Extraction and rendering will consume this model in later slices.
#[allow(dead_code)]
mod model;
// Builtin and custom extractors will consume this rule contract later.
#[allow(dead_code)]
mod rule;

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
