mod common;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
use common::create_test_files;
use predicates::str::contains;

#[test]
fn test_sg_prints_deprecation_warning() -> Result<()> {
  let ast_grep = cargo_bin!("ast-grep");
  let mut paths = vec![ast_grep.parent().unwrap().to_path_buf()];
  if let Some(path) = std::env::var_os("PATH") {
    paths.extend(std::env::split_paths(&path));
  }
  Command::new(cargo_bin!("sg"))
    .env("PATH", std::env::join_paths(paths)?)
    .arg("--version")
    .assert()
    .success()
    .stderr(contains(
      "WARNING: `sg` is deprecated. Use `ast-grep` instead.",
    ));
  Ok(())
}

#[test]
fn test_help_work_for_invalid_sgconfig() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", "invalid")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["help"])
    .assert()
    .success()
    .stdout(contains("ast-grep"));
  Ok(())
}
