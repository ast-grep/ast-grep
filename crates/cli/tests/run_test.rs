mod common;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
use common::create_test_files;
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn test_simple_infer_lang() -> Result<()> {
  let dir = create_test_files([("a.ts", "console.log(123)"), ("b.rs", "console.log(456)")])?;
  Command::new(cargo_bin!())
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
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "console.log($A)", "-l", "rs"])
    .assert()
    .success()
    .stdout(contains("console.log(123)").not())
    .stdout(contains("console.log(456)"));
  Ok(())
}

#[test]
fn test_pattern_output_highlights_only_consumed_range() -> Result<()> {
  let source = "let a = 123 /* comment */;";
  let highlighted = "\x1b[1;31mlet a = 123\x1b[0m /* comment */;";
  for heading in ["always", "never"] {
    Command::new(cargo_bin!())
      .args([
        "run",
        "--stdin",
        "-p",
        "let a = 123",
        "-l",
        "js",
        "--heading",
        heading,
        "--color",
        "ansi",
      ])
      .write_stdin(source)
      .assert()
      .success()
      .stdout(contains(highlighted));
  }
  Ok(())
}

#[test]
fn test_pattern_output_uses_consumed_range_for_context() -> Result<()> {
  Command::new(cargo_bin!())
    .args([
      "run",
      "--stdin",
      "-p",
      "struct $A: $B",
      "-l",
      "cpp",
      "--heading",
      "never",
      "--color",
      "ansi",
    ])
    .write_stdin("struct A: B {\n  int value;\n};")
    .assert()
    .success()
    .stdout(contains("\x1b[1;31mstruct A: B\x1b[0m {"))
    .stdout(contains("int value").not());
  Ok(())
}

#[test]
fn test_pattern_output_merges_consumed_ranges() -> Result<()> {
  Command::new(cargo_bin!())
    .args([
      "run",
      "--stdin",
      "-p",
      "let $A = $B",
      "-l",
      "js",
      "--heading",
      "never",
      "--color",
      "ansi",
    ])
    .write_stdin("let a = 1 /* first */; let b = 2 /* second */;")
    .assert()
    .success()
    .stdout(
      "STDIN:1:\x1b[1;31mlet a = 1\x1b[0m /* first */; \x1b[1;31mlet b = 2\x1b[0m /* second */;\n",
    );
  Ok(())
}

#[test]
fn test_pattern_json_keeps_full_node_range() -> Result<()> {
  Command::new(cargo_bin!())
    .args([
      "run",
      "--stdin",
      "-p",
      "let a = 123",
      "-l",
      "js",
      "--json=compact",
    ])
    .write_stdin("let a = 123 /* comment */;")
    .assert()
    .success()
    .stdout(contains(r#""text":"let a = 123 /* comment */;""#))
    .stdout(contains(r#""end":26"#));
  Ok(())
}

#[test]
fn test_kind_selector() -> Result<()> {
  let dir = create_test_files([("a.js", "test(123)\nconst test = 456")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["run", "-k", "call_expression > identifier", "-l", "js"])
    .assert()
    .success()
    .stdout(contains("test(123)"))
    .stdout(contains("const test = 456").not());
  Ok(())
}

#[test]
fn test_default_run_with_kind_selector() -> Result<()> {
  let dir = create_test_files([("a.js", "test(123)\nconst test = 456")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-k", "call_expression > identifier", "-l", "js"])
    .assert()
    .success()
    .stdout(contains("test(123)"))
    .stdout(contains("const test = 456").not());
  Ok(())
}

#[test]
fn test_kind_selector_error_context() -> Result<()> {
  let dir = create_test_files([("a.js", "test(123)")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["run", "-k", "call_expression >", "-l", "js"])
    .assert()
    .failure()
    .stderr(contains("Cannot parse kind as a valid selector."));
  Ok(())
}

#[test]
fn test_js_in_html() -> Result<()> {
  let dir = create_test_files([
    ("a.html", "<script>alert(1)</script>"),
    ("b.js", "alert(456)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-l", "js"])
    .assert()
    .success()
    .stdout(contains("alert(1)"))
    .stdout(contains("alert(456)"));
  Ok(())
}

#[test]
fn test_outline_javascript_in_vue_as_html() -> Result<()> {
  let dir = create_test_files([
    (
      "sgconfig.yml",
      r#"ruleDirs: []
languageGlobs:
  html:
    - "*.vue"
"#,
    ),
    (
      "component.vue",
      r#"<template><main>Hello</main></template>
<script lang="typescript">
export function greet(name: string) {
  return `Hello ${name}`;
}
</script>"#,
    ),
  ])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["outline", "component.vue", "--json=compact"])
    .assert()
    .success()
    .stdout(contains(r#""language":"Html""#))
    .stdout(contains(r#""name":"greet""#));
  Ok(())
}

#[test]
fn test_outline_javascript_in_html_stdin() -> Result<()> {
  Command::new(cargo_bin!())
    .args(["outline", "--stdin", "--lang", "html", "--json=compact"])
    .write_stdin(
      r#"<script lang="typescript">
export function greet(name: string) {
  return `Hello ${name}`;
}
</script>"#,
    )
    .assert()
    .success()
    .stdout(contains(r#""path":"STDIN""#))
    .stdout(contains(r#""language":"Html""#))
    .stdout(contains(r#""name":"greet""#));
  Ok(())
}

#[test]
fn test_rewrite_js_in_html() -> Result<()> {
  let dir = create_test_files([("a.html", "<script>alert(1)</script>")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-r", "alert(456)"])
    .assert()
    .success()
    .stdout(contains("alert(1)"))
    .stdout(contains("alert(456)"));
  Ok(())
}

#[test]
fn test_inspect() -> Result<()> {
  let dir = create_test_files([("a.js", "alert(1)"), ("b.js", "alert(456)")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-l", "js", "--inspect", "entity"])
    .assert()
    .success()
    .stdout(contains("alert(1)"))
    .stderr(contains("scannedFileCount=2"));
  Ok(())
}

#[test]
fn test_status_code_fail_with_no_match() -> Result<()> {
  let dir = create_test_files([("a.js", "alert(1)")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "no-match"])
    .assert()
    .failure()
    .stdout(predicate::str::is_empty());
  Ok(())
}

#[test]
fn test_debug_query() -> Result<()> {
  // should not print pattern if invalid
  Command::new(cargo_bin!())
    .args(["-p", "foo;bar;", "-l", "js", "--debug-query"])
    .assert()
    .failure()
    .stderr(contains("Debug Pattern").not())
    .stderr(contains("Cannot parse query as a valid pattern"));

  // should  print debug tree even for invalid pattern
  Command::new(cargo_bin!())
    .args(["-p", "foo;bar;", "-l", "js", "--debug-query=ast"])
    .assert()
    .failure()
    .stderr(contains("Debug AST"))
    .stderr(contains("Cannot parse query as a valid pattern"));

  Ok(())
}

#[test]
fn test_unsupport_config_arg() -> Result<()> {
  let dir = create_test_files([])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-c", "not-found.yml"])
    .assert()
    .failure()
    .stderr(contains("unexpected argument"));
  Ok(())
}

#[test]
fn test_trace_default_project() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", "ruleDirs: []")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "--inspect=summary"])
    .assert()
    .failure()
    .stderr(contains("isProject=true,projectDir"));
  Ok(())
}

#[test]
fn test_trace_project() -> Result<()> {
  let dir = create_test_files([("not.yml", "ruleDirs: []")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "--inspect=summary"])
    .assert()
    .failure()
    .stderr(contains("isProject=false"));
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["run", "-c=not.yml", "-p", "alert($A)", "--inspect=summary"])
    .assert()
    .failure()
    .stderr(contains("isProject=true,projectDir"));
  Ok(())
}
