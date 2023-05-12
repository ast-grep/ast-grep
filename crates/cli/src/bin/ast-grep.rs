// This file is exactly the same as main.rs
// we need this to avoid "multiple build target" warning
// See https://github.com/rust-lang/cargo/issues/5930
use anyhow::Result;
use ast_grep::execute_main;

fn main() -> Result<()> {
  execute_main()
}
