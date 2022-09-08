#![feature(let_chains)]

mod config;
mod interaction;
mod languages;
mod lsp;
mod print;
mod scan;
mod test;

use clap::{Parser, Subcommand};
use scan::{run_with_config, run_with_pattern, RunArg, ScanArg};
use std::io::Result;
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
  if std::env::args().nth(1).unwrap_or_default().starts_with('-') {
    // handle no subcommand
    let arg = RunArg::parse();
    return run_with_pattern(arg);
  }
  let app = App::parse();
  match app.command {
    Commands::Run(arg) => run_with_pattern(arg),
    Commands::Scan(arg) => run_with_config(arg),
    Commands::Test(arg) => run_test_rule(arg),
    Commands::Lsp => lsp::run_language_server(),
    Commands::Docs => todo!("todo, generate rule docs based on current config"),
  }
}
