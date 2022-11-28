mod config;
mod error;
mod interaction;
mod languages;
mod lsp;
mod print;
mod scan;
mod test;

use anyhow::Result;
use clap::{Parser, Subcommand};

use error::exit_with_error;
use scan::{run_with_config, run_with_pattern, RunArg, ScanArg};
use test::{run_test_rule, TestArg};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
/**
 * TODO: add some description for ast-grep: sg
 * Example:
 * sg -p "$PATTERN.to($MATCH)" -l ts --rewrite "use($MATCH)"
 */
struct App {
  #[clap(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Run one time search or rewrite in command line. (default command)
  #[clap(display_order = 1)]
  Run(RunArg),
  /// Scan and rewrite code by configuration
  #[clap(display_order = 2)]
  Scan(ScanArg),
  /// test ast-grep rule
  #[clap(display_order = 3)]
  Test(TestArg),
  /// starts language server
  #[clap(display_order = 4)]
  Lsp,
  /// generate rule docs for current configuration
  #[clap(display_order = 5)]
  Docs,
}

fn main() -> Result<()> {
  match main_with_args(std::env::args()) {
    Err(error) => exit_with_error(error),
    ok => ok,
  }
}

// this wrapper function is for testing
fn main_with_args(args: impl Iterator<Item = String>) -> Result<()> {
  let args: Vec<_> = args.collect();
  if let Some(arg) = args.get(1) {
    if arg.starts_with('-') {
      // handle no subcommand
      let arg = RunArg::try_parse_from(args)?;
      return run_with_pattern(arg);
    }
  }
  let app = App::try_parse_from(args)?;
  match app.command {
    Commands::Run(arg) => run_with_pattern(arg),
    Commands::Scan(arg) => run_with_config(arg),
    Commands::Test(arg) => run_test_rule(arg),
    Commands::Lsp => lsp::run_language_server(),
    Commands::Docs => todo!("todo, generate rule docs based on current config"),
  }
}

#[cfg(test)]
mod test_cli {
  use super::*;

  fn sg(args: impl IntoIterator<Item = &'static str>) -> Result<()> {
    main_with_args(std::iter::once("sg".into()).chain(args.into_iter().map(|s| s.to_string())))
  }

  fn wrong_usage(args: impl IntoIterator<Item = &'static str>) {
    let err = sg(args).unwrap_err();
    assert!(err.downcast::<clap::Error>().is_ok());
  }

  #[test]
  fn test_wrong_usage() {
    wrong_usage([]);
    wrong_usage(["Some($A)", "-l", "rs"]);
    wrong_usage(["-l", "rs"]);
  }

  #[test]
  fn test_default_subcommand() {
    assert!(sg(["-p", "Some($A)", "-l", "rs"]).is_ok());
    assert!(sg(["-p", "Some($A)"]).is_ok()); // inferred lang
    assert!(sg(["-p", "Some($A)", "-l", "rs", "-r", "$A.unwrap()"]).is_ok());
  }
}
