mod common;

use std::process::ExitCode;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
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

#[test]
fn test_sg_test_unmatched_test_id() -> Result<()> {
  for (test, skip_snapshots, summary) in [
    (TEST, true, "test result: ok. 1 passed; 0 failed;"),
    (WRONG_TEST, true, "test failed. 0 passed; 1 failed;"),
    (TEST, false, "test failed. 0 passed; 1 failed;"),
  ] {
    let dir = create_test_files([
      ("sgconfig.yml", CONFIG),
      ("rules/test-rule.yml", RULE),
      ("rule-tests/test-rule-test.yml", test),
      (
        "rule-tests/renamed-test.yml",
        "id: no-such-rule\nvalid: ['a']\ninvalid: ['b']",
      ),
    ])?;
    let mut command = Command::new(cargo_bin!());
    command
      .current_dir(dir.path())
      .args(["test", "--color", "never"]);
    if skip_snapshots {
      command.arg("--skip-snapshot-tests");
    }
    command
      .assert()
      .code(4)
      .stdout(contains(summary))
      .stderr(contains("1 test case(s) matched no rule."));
  }
  Ok(())
}

#[test]
fn test_sg_test_unmatched_test_id_with_off_rule() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", OFF_RULE),
    ("rule-tests/test-rule-test.yml", WRONG_TEST),
    (
      "rule-tests/renamed-test.yml",
      "id: no-such-rule\nvalid: ['a']\ninvalid: ['b']",
    ),
  ])?;
  let config = dir.path().join("sgconfig.yml");
  let error = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ))
  .unwrap_err();
  assert_eq!(error.to_string(), "1 test case(s) matched no rule.");
  drop(dir);
  Ok(())
}

#[cfg(unix)]
#[test]
fn test_sg_test_follow_symlinked_test_dir() -> Result<()> {
  use std::os::unix::fs::symlink;

  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", RULE),
    ("rule-tests/.keep", ""),
    ("real-rule-tests/test-rule-test.yml", WRONG_TEST),
  ])?;
  symlink(
    dir.path().join("real-rule-tests"),
    dir.path().join("rule-tests").join("linked"),
  )?;

  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());

  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests --follow",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}
