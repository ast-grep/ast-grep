mod common;

use std::process::ExitCode;

use anyhow::Result;
use assert_cmd::{cargo_bin, Command};
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

fn sg(s: &str) -> Result<ExitCode> {
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
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rules/rule.yml"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--filter", "on-rule"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--off", "on-rule"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--off"])
    .assert()
    .success();
  Ok(())
}

#[test]
fn test_severity_override() -> Result<()> {
  let dir = setup()?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--error"])
    .assert()
    .failure()
    .stdout(contains("error"));
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--error=on-rule"])
    .assert()
    .failure()
    .stdout(contains("error"));
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("print").not())
    .stdout(contains("transform-indent"));
  Ok(())
}

const LABEL_RULE: &str = r"
id: label-test
language: TypeScript
rule: { all: [pattern: Some($A), pattern: $B] }
labels:
  A:
    style: primary
    message: primary-label
  B:
    style: secondary
    message: secondary-label
";

#[test]
fn test_label() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", LABEL_RULE),
    ("test.ts", "Some(123) + None"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("primary-label"))
    .stdout(contains("secondary-label"))
    .stdout(contains(" -----^^^-")) // a label range test
    .stdout(contains(" -----^^^--").not());
  Ok(())
}
const FILE_RULE: &str = "
id: file-rule
message: test rule
language: TypeScript
rule: { pattern: Some($A) }
files: [ test/*.ts ]
";

#[test]
fn test_file() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", FILE_RULE),
    ("test/hit.ts", "Some(123)"),
    ("not.ts", "Some(456)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path().join("test"))
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("hit.ts"))
    .stdout(contains("not.ts").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path().join("test"))
    .args(["scan", "-c", "../sgconfig.yml"])
    .assert()
    .success()
    .stdout(contains("hit.ts"))
    .stdout(contains("not.ts").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-c", "sgconfig.yml"])
    .assert()
    .success()
    .stdout(contains("hit.ts"))
    .stdout(contains("not.ts").not());
  Ok(())
}

const MAX_DIAG_RULE: &str = "
id: max-result-rule
message: test rule
severity: warning
language: TypeScript
rule: { pattern: Some($A) }
";

#[test]
fn test_max_diagnostics_shown() -> Result<()> {
  // Create 4 files, each with one match
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", MAX_DIAG_RULE),
    ("a.ts", "Some(1)"),
    ("b.ts", "Some(2)"),
    ("c.ts", "Some(3)"),
    ("d.ts", "Some(4)"),
  ])?;
  // With --max-results=2, should only output 2 matches
  let output = Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--json", "--max-results=2"])
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(matches.len(), 2, "should output exactly 2 matches");
  Ok(())
}

#[test]
fn test_max_diagnostics_shown_single_file() -> Result<()> {
  // Single file with 4 matches
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", MAX_DIAG_RULE),
    ("test.ts", "Some(1); Some(2); Some(3); Some(4)"),
  ])?;
  // With --max-results=2, should only output 2 matches
  let output = Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--json", "--max-results=2"])
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(
    matches.len(),
    2,
    "should output exactly 2 matches from single file"
  );
  Ok(())
}

#[test]
fn test_max_diagnostics_shown_stdin() -> Result<()> {
  // Test --max-results with stdin input
  let input = "Some(1); Some(2); Some(3); Some(4)";
  let output = Command::new(cargo_bin!())
    .args([
      "scan",
      "--stdin",
      "--json",
      "--inline-rules",
      MAX_DIAG_RULE,
      "--max-results=2",
    ])
    .write_stdin(input)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(
    matches.len(),
    2,
    "should output exactly 2 matches from stdin"
  );
  Ok(())
}

#[test]
fn test_yaml_sgconfig_extension() -> Result<()> {
  let dir = create_test_files([("sgconfig.yaml", CONFIG), ("rules/rule.yml", FILE_RULE)])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success();
  Ok(())
}

#[test]
fn test_sg_scan_sarif_output() -> Result<()> {
  let dir = setup()?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--format", "sarif"])
    .assert()
    .success()
    .stdout(contains("\"version\""))
    .stdout(contains("\"runs\""))
    .stdout(contains("\"results\""))
    .stdout(contains("\"ruleId\": \"on-rule\""))
    .stdout(predicate::function(|output: &str| {
      // Verify it's valid JSON
      from_slice::<Value>(output.as_bytes()).is_ok()
    }));
  Ok(())
}

#[test]
fn test_sg_scan_sarif_with_fixes() -> Result<()> {
  let rule = "
id: use-let
message: Use let instead of var
severity: error
language: JavaScript
rule:
  pattern: var $VAR = $VAL
fix: let $VAR = $VAL
";
  let dir = create_test_files([("rule.yml", rule), ("test.js", "var x = 123;")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--format", "sarif"])
    .assert()
    .stdout(contains("\"fixes\""))
    .stdout(contains("\"artifactChanges\""))
    .stdout(contains("\"replacements\""))
    .stdout(contains("\"deletedRegion\""))
    .stdout(contains("\"insertedContent\""))
    .stdout(predicate::function(|output: &str| {
      from_slice::<Value>(output.as_bytes()).is_ok()
    }));
  Ok(())
}

#[test]
fn test_status_code_success_with_no_match() -> Result<()> {
  let dir = create_test_files([("rule.yml", RULE1)])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .stdout(predicate::str::is_empty())
    .success();
  Ok(())
}

#[test]
fn test_scan_inline_rules_no_id() -> Result<()> {
  Command::new(cargo_bin!())
    .args([
      "scan",
      "--stdin",
      "--inline-rules",
      "{language: ts, rule: {pattern: console.log($A)}}",
      "--json",
    ])
    .write_stdin("console.log(123)")
    .assert()
    .success()
    .stdout(contains("\"text\": \"console.log(123)\""));
  Ok(())
}

#[test]
fn test_scan_rule_id_defaults_to_filename() -> Result<()> {
  let rule = "
language: TypeScript
rule: { pattern: Some($A) }
";
  let dir = create_test_files([("no-some-call.yml", rule), ("test.ts", "Some(123)")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "no-some-call.yml", "--json"])
    .assert()
    .success()
    .stdout(contains("no-some-call"));
  Ok(())
}

#[test]
fn test_scan_explicit_id_not_overwritten() -> Result<()> {
  let rule = "
id: my-explicit-id
language: TypeScript
rule: { pattern: Some($A) }
";
  let dir = create_test_files([("other-name.yml", rule), ("test.ts", "Some(123)")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "other-name.yml", "--json"])
    .assert()
    .success()
    .stdout(contains("my-explicit-id"));
  Ok(())
}

#[test]
fn test_scan_multi_rule_file_auto_numbered_ids() -> Result<()> {
  let rules = "
language: TypeScript
rule: { pattern: Some($A) }
---
language: TypeScript
rule: { pattern: None }
";
  let dir = create_test_files([("my-rules.yml", rules), ("test.ts", "Some(123)\nNone")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "my-rules.yml", "--json"])
    .assert()
    .success()
    .stdout(contains("my-rules-0"))
    .stdout(contains("my-rules-1"));
  Ok(())
}

#[test]
fn test_scan_multi_rule_file_mixed_ids() -> Result<()> {
  let rules = "
id: first-rule
language: TypeScript
rule: { pattern: Some($A) }
---
language: TypeScript
rule: { pattern: None }
---
id: third-rule
language: TypeScript
rule: { pattern: 'hello' }
";
  let dir = create_test_files([
    ("my-rules.yml", rules),
    ("test.ts", "Some(123)\nNone\nhello"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "my-rules.yml", "--json"])
    .assert()
    .success()
    .stdout(contains("first-rule"))
    .stdout(contains("my-rules-1"))
    .stdout(contains("third-rule"))
    .stdout(contains("my-rules-0").not())
    .stdout(contains("my-rules-2").not());
  Ok(())
}

#[test]
fn test_scan_multi_rule_file_with_explicit_ids() -> Result<()> {
  let rules = "
id: find-some
language: TypeScript
rule: { pattern: Some($A) }
---
id: find-none
language: TypeScript
rule: { pattern: None }
";
  let dir = create_test_files([("my-rules.yml", rules), ("test.ts", "Some(123)\nNone")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "my-rules.yml", "--json"])
    .assert()
    .success()
    .stdout(contains("find-some"))
    .stdout(contains("find-none"));
  Ok(())
}

#[test]
fn test_scan_duplicate_default_ids() -> Result<()> {
  let rule = "
language: TypeScript
rule: { pattern: Some($A) }
";
  let dir = create_test_files([
    ("sgconfig.yml", "ruleDirs:\n- rules"),
    ("rules/check.yml", rule),
    ("rules/check.yaml", rule),
    ("test.ts", "Some(123)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .failure()
    .stderr(contains("Duplicate rule id `check`"));
  Ok(())
}

#[cfg(unix)]
#[test]
fn test_scan_invalid_rule_id() -> Result<()> {
  use std::ffi::OsStr;
  use std::os::unix::ffi::OsStrExt;
  let dir = TempDir::new()?;
  let rules_dir = dir.path().join("rules");
  std::fs::create_dir_all(&rules_dir)?;
  std::fs::write(dir.path().join("sgconfig.yml"), "ruleDirs:\n- rules")?;
  std::fs::write(
    rules_dir.join(OsStr::from_bytes(b"\xff.yml")),
    "language: TypeScript\nrule: { pattern: Some($A) }",
  )?;
  std::fs::write(dir.path().join("test.ts"), "Some(123)")?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .failure()
    .stderr(contains("Cannot infer rule id"));
  Ok(())
}
