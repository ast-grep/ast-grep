mod common;

use anyhow::Result;
use assert_cmd::Command;
use common::create_test_files;
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn test_simple_infer_lang() -> Result<()> {
  let dir = create_test_files([("a.ts", "console.log(123)"), ("b.rs", "console.log(456)")])?;
  Command::cargo_bin("sg")?
    .current_dir(dir.path())
    .args(["-p", "console.log($A)"])
    .assert()
    .success()
    .stdout(contains("console.log(123)"))
    .stdout(contains("console.log(456)"));
  Ok(())
}

#[test]
fn test_simple_specific_lang() -> Result<()> {
  let dir = create_test_files([("a.ts", "console.log(123)"), ("b.rs", "console.log(456)")])?;
  Command::cargo_bin("sg")?
    .current_dir(dir.path())
    .args(["-p", "console.log($A)", "-l", "rs"])
    .assert()
    .success()
    .stdout(contains("console.log(123)").not())
    .stdout(contains("console.log(456)"));
  Ok(())
}

#[test]
fn test_js_in_html() -> Result<()> {
  let dir = create_test_files([
    ("a.html", "<script>alert(1)</script>"),
    ("b.js", "alert(456)"),
  ])?;
  Command::cargo_bin("sg")?
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-l", "js"])
    .assert()
    .success()
    .stdout(contains("alert(1)"))
    .stdout(contains("alert(456)"));
  Ok(())
}

#[test]
fn test_inspect() -> Result<()> {
  let dir = create_test_files([("a.js", "alert(1)"), ("b.js", "alert(456)")])?;
  Command::cargo_bin("sg")?
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-l", "js", "--inspect", "entity"])
    .assert()
    .success()
    .stdout(contains("alert(1)"))
    .stderr(contains("scannedFileCount=2"));
  Ok(())
}

#[test]
fn test_debug_query() -> Result<()> {
  // should not print pattern if invalid
  Command::cargo_bin("sg")?
    .args(["-p", "foo;bar;", "-l", "js", "--debug-query"])
    .assert()
    .failure()
    .stderr(contains("Debug Pattern").not())
    .stderr(contains("Cannot parse query as a valid pattern"));

  // should  print debug tree even for invalid pattern
  Command::cargo_bin("sg")?
    .args(["-p", "foo;bar;", "-l", "js", "--debug-query=ast"])
    .assert()
    .failure()
    .stderr(contains("Debug AST"))
    .stderr(contains("Cannot parse query as a valid pattern"));

  Ok(())
}
