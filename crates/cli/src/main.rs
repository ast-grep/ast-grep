use std::process::ExitCode;

use anyhow::Result;
use ast_grep::execute_main;

fn main() -> Result<ExitCode> {
  execute_main()
}
