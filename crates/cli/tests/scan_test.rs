mod common;

use anyhow::Result;
use assert_cmd::Command;
use ast_grep::main_with_args;
use common::create_test_files;
use predicates::prelude::*;
use predicates::str::contains;
use serde_json::{from_slice, Value};
use tempfile::TempDir;

const CONFIG: &str = "
ruleDirs:
- rules
testConfigs:
- testDir: rule-tests
";
const RULE1: &str = "
id: on-rule
message: test rule
severity: warning
language: TypeScript
rule:
  pattern: Some($A)
";

const RULE2: &str = "
id: off-rule
severity: off
language: TypeScript
rule:
  pattern: Some($A)
";

fn setup() -> Result<TempDir> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/on-rule.yml", RULE1),
    ("rules/off-rule.yml", RULE2),
    ("test.ts", "Some(123)"),
  ])?;
  assert!(dir.path().join("sgconfig.yml").exists());
  Ok(dir)
}

fn sg(s: &str) -> Result<()> {
  let args = s.split(' ').map(String::from);
  main_with_args(args)
}

#[test]
fn test_sg_scan() -> Result<()> {
  let dir = setup()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!("ast-grep scan -c {}", config.display()));
  assert!(ret.is_ok());
  let ret = sg(&format!("ast-grep scan -c={}", config.display()));
  assert!(ret.is_ok());
  let ret = sg(&format!("ast-grep scan --config {}", config.display()));
  assert!(ret.is_ok());
  let ret = sg(&format!("ast-grep scan --config={}", config.display()));
  assert!(ret.is_ok());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_rule_off() -> Result<()> {
  let dir = setup()?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("on-rule"))
    .stdout(contains("off-rule").not());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_scan_inline_rules() -> Result<()> {
  let inline_rules = "{id: test, language: ts, rule: {pattern: console.log($A)}}";
  Command::cargo_bin("ast-grep")?
    .args(["scan", "--stdin", "--inline-rules", inline_rules, "--json"])
    .write_stdin("console.log(123)")
    .assert()
    .stdout(contains("\"text\": \"console.log(123)\""))
    .stdout(predicate::function(|n| from_slice::<Value>(n).is_ok()));
  Ok(())
}

const MULTI_RULES: &str = "
id: rule-1
language: TypeScript
rule: { pattern: Some($A) }
---
id: rule-2
language: TypeScript
rule: { pattern: None }
";

#[test]
fn test_sg_scan_multiple_rules_in_one_file() -> Result<()> {
  let dir = create_test_files([("rule.yml", MULTI_RULES), ("test.ts", "Some(123) + None")])?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("rule-1"))
    .stdout(contains("rule-2"))
    .stdout(contains("rule-3").not());
  Ok(())
}

// see #517, #668
#[test]
fn test_sg_scan_py_empty_text() -> Result<()> {
  let inline_rules = "{id: test, language: py, rule: {pattern: None}}";
  Command::cargo_bin("ast-grep")?
    .args(["scan", "--stdin", "--inline-rules", inline_rules])
    .write_stdin("\n\n\n\n\nNone")
    .assert()
    .stdout(contains("STDIN:6:1"));
  Ok(())
}

#[test]
fn test_sg_scan_html() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", RULE1),
    ("test.html", "<script lang=ts>Some(123)</script>"),
  ])?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--inspect=summary"])
    .assert()
    .success()
    .stdout(contains("on-rule"))
    .stdout(contains("script"))
    .stdout(contains("rule-3").not())
    .stderr(contains("scannedFileCount=1"));
  Ok(())
}

#[test]
fn test_scan_unused_suppression() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", RULE1),
    ("test.ts", "None(123) // ast-grep-ignore"),
  ])?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("unused-suppression"));
  Ok(())
}

#[test]
fn test_unused_suppression_only_in_scan() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", RULE1),
    ("test.ts", "None(123) // ast-grep-ignore"),
  ])?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "-r", "rules/rule.yml"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--filter", "on-rule"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--off", "on-rule"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--inline-rules", RULE1])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Ok(())
}

#[test]
fn test_scan_unused_suppression_off() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", RULE1),
    ("test.ts", "None(123) // ast-grep-ignore"),
  ])?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--off"])
    .assert()
    .success();
  Ok(())
}

#[test]
fn test_severity_override() -> Result<()> {
  let dir = setup()?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--error"])
    .assert()
    .failure()
    .stdout(contains("error"));
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--error=on-rule"])
    .assert()
    .failure()
    .stdout(contains("error"));
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan", "--error=not-exist"])
    .assert()
    .success()
    .stdout(contains("warning"));
  Ok(())
}

const PY_RULE: &str = r"
id: transform-indent
language: python
rule: { pattern: 'class $CN(): $A' }
transform:
  AR:
    substring: { source: $A }
fix: |-
  class $CN():
      $AR
";

const PY_FILE: &str = r"
if something:
    class B():
        def replace(self):
        print(self1)
";

#[test]
fn test_transform_indent() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", PY_RULE),
    ("test.py", PY_FILE),
  ])?;
  Command::cargo_bin("ast-grep")?
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("print").not())
    .stdout(contains("transform-indent"));
  Ok(())
}
