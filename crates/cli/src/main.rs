mod config;
mod error;
mod interaction;
mod lsp;
mod print;
mod scan;
mod verify;

use anyhow::Result;
use clap::{Parser, Subcommand};

use error::exit_with_error;
use scan::{run_with_config, run_with_pattern, RunArg, ScanArg};
use verify::{run_test_rule, TestArg};

const LOGO: &str = r#"
Search and Rewrite code at large scale using AST pattern.
                    __
        ____ ______/ /_      ____ _________  ____
       / __ `/ ___/ __/_____/ __ `/ ___/ _ \/ __ \
      / /_/ (__  ) /_/_____/ /_/ / /  /  __/ /_/ /
      \__,_/____/\__/      \__, /_/   \___/ .___/
                          /____/         /_/
"#;
#[derive(Parser)]
#[clap(author, version, about, long_about = LOGO)]
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
  Run(RunArg),
  /// Scan and rewrite code by configuration
  Scan(ScanArg),
  /// test ast-grep rule
  Test(TestArg),
  /// starts language server
  Lsp,
  /// generate rule docs for current configuration
  Docs,
}

fn main() -> Result<()> {
  match main_with_args(std::env::args()) {
    Err(error) => exit_with_error(error),
    ok => ok,
  }
}

fn try_default_run(args: &[String]) -> Result<Option<RunArg>> {
  // use `run` if there is at lease one pattern arg with no user provided command
  let should_use_default_run_command =
    args.iter().skip(1).any(|p| p == "-p" || p == "--pattern") && args[1].starts_with('-');
  if should_use_default_run_command {
    // handle no subcommand
    let arg = RunArg::try_parse_from(args)?;
    Ok(Some(arg))
  } else {
    Ok(None)
  }
}

// this wrapper function is for testing
fn main_with_args(args: impl Iterator<Item = String>) -> Result<()> {
  let args: Vec<_> = args.collect();
  if let Some(arg) = try_default_run(&args)? {
    return run_with_pattern(arg);
  }
  let app = App::try_parse_from(args)?;
  // TODO: add test for app parse
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

  fn sg(args: &str) -> Result<App> {
    let app = App::try_parse_from(
      std::iter::once("sg".into()).chain(args.split(' ').map(|s| s.to_string())),
    )?;
    Ok(app)
  }

  fn ok(args: &str) -> App {
    sg(args).expect("should parse")
  }
  fn error(args: &str) -> clap::Error {
    let Err(err) = sg(args) else {
      panic!("app parsing should fail!")
    };
    err
      .downcast::<clap::Error>()
      .expect("should have clap::Error")
  }

  #[test]
  fn test_wrong_usage() {
    error("");
    error("Some($A) -l rs");
    error("-l rs");
  }

  #[test]
  fn test_version_and_help() {
    let version = error("--version");
    assert!(version.to_string().starts_with("ast-grep"));
    let version = error("-V");
    assert!(version.to_string().starts_with("ast-grep"));
    let help = error("--help");
    assert!(help.to_string().contains("Search and Rewrite code"));
  }

  fn default_run(args: &str) {
    let args: Vec<_> = std::iter::once("sg".into())
      .chain(args.split(' ').map(|s| s.to_string()))
      .collect();
    assert!(matches!(try_default_run(&args), Ok(Some(_))));
  }
  #[test]
  fn test_default_subcommand() {
    default_run("-p Some($A) -l rs");
    default_run("-p Some($A)");
    default_run("-p Some($A) -l rs -r $A.unwrap()");
  }

  #[test]
  fn test_run() {
    ok("run -p test -i");
    ok("run -p test --interactive dir");
    ok("run -p test -r Test dir");
    ok("run -p test -l rs --debug-query");
    ok("run -p test -l rs --color always");
    ok("run -p test -l rs --heading always");
    ok("run -p test dir1 dir2 dir3"); // multiple paths
    error("run test");
    error("run --debug-query test"); // missing lang
    error("run -r Test dir");
    error("run -p test -i --json dir"); // conflict
    error("run -p test -l rs -c always"); // no color shortcut
  }

  #[test]
  fn test_scan() {
    ok("scan");
    ok("scan dir");
    ok("scan -r test-rule.yml dir");
    ok("scan -c test-rule.yml dir");
    ok("scan -c test-rule.yml");
    ok("scan --report-style short"); // conflict
    ok("scan dir1 dir2 dir3"); // multiple paths
    error("scan -i --json dir"); // conflict
    error("scan --report-style rich --json dir"); // conflict
    error("scan -r test.yml -c test.yml --json dir"); // conflict
  }
}
