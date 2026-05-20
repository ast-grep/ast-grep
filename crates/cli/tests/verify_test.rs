mod common;

use std::process::ExitCode;

use anyhow::Result;
use assert_cmd::{cargo_bin, Command};
use ast_grep::main_with_args;
use common::create_test_files;
use predicates::str::contains;
use tempfile::TempDir;

const CONFIG: &str = "
ruleDirs:
- rules
testConfigs:
- testDir: rule-tests
";
const RULE: &str = "
id: test-rule
message: test rule
severity: warning
language: TypeScript
rule:
  pattern: Some($A)
";
const OFF_RULE: &str = "
id: test-rule
message: test rule
severity: off
language: TypeScript
rule:
  pattern: Some($A)
";

const TEST: &str = "
id: test-rule
valid:
- None
invalid:
- Some(123)
";

const WRONG_TEST: &str = "
id: test-rule
valid:
- Some(123)
invalid:
- None
";

fn setup() -> Result<TempDir> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", RULE),
    ("rule-tests/test-rule-test.yml", TEST),
    ("test.ts", "Some(123)"),
  ])?;
  assert!(dir.path().join("sgconfig.yml").exists());
  Ok(dir)
}

fn sg(s: &str) -> Result<ExitCode> {
  let args = s.split(' ').map(String::from);
  main_with_args(args)
}

#[cfg(unix)]
fn create_symlink(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
  std::os::unix::fs::symlink(src, dst)?;
  Ok(())
}

#[cfg(windows)]
fn create_symlink(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
  std::os::windows::fs::symlink_file(src, dst)?;
  Ok(())
}

#[test]
fn test_sg_test() -> Result<()> {
  let dir = setup()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_test_follow_symlink() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", RULE),
    ("rule-tests/.keep", ""),
    ("real-tests/test-rule-test.yml", TEST),
    ("test.ts", "Some(123)"),
  ])?;
  create_symlink(
    &dir.path().join("real-tests/test-rule-test.yml"),
    &dir.path().join("rule-tests/test-rule-test.yml"),
  )?;
  let config = dir.path().join("sgconfig.yml");
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "test",
      "-c",
      &format!("{}", config.display()),
      "--skip-snapshot-tests",
    ])
    .assert()
    .success()
    .stdout(contains("Running 0 tests"));
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "test",
      "-c",
      &format!("{}", config.display()),
      "--skip-snapshot-tests",
      "--follow",
    ])
    .assert()
    .success()
    .stdout(contains("Running 1 tests"));
  Ok(())
}

fn setup_error() -> Result<TempDir> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", RULE),
    ("rule-tests/test-rule-test.yml", WRONG_TEST),
    ("test.ts", "Some(123)"),
  ])?;
  assert!(dir.path().join("sgconfig.yml").exists());
  Ok(dir)
}

#[test]
fn test_sg_test_error() -> Result<()> {
  let dir = setup_error()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}

// should skip/pick wrong_test based on filter
#[test]
fn test_sg_test_filter() -> Result<()> {
  let dir = setup_error()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests -f error-rule",
    config.display()
  ));
  assert!(ret.is_err());
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests -f test-rule",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_test_off_rule() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", OFF_RULE),
    ("rule-tests/test-rule-test.yml", WRONG_TEST),
    ("test.ts", "Some(123)"),
  ])?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests --include-off",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}
