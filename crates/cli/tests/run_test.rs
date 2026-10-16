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
fn test_outline_markdown_headings() -> Result<()> {
  let dir = create_test_files([(
    "guide.md",
    r#"# Introduction

Usage
=====

###### Details

References
----------

```markdown
# Not a heading
```
"#,
  )])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["outline", "guide.md", "--json=compact"])
    .assert()
    .success()
    .stdout(contains(r#""language":"Markdown""#))
    .stdout(contains(r#""name":"Introduction""#))
    .stdout(contains(r#""name":"Usage""#))
    .stdout(contains(r#""name":"Details""#))
    .stdout(contains(r#""name":"References""#))
    .stdout(contains(r#""name":"Not a heading""#).not());
  Ok(())
}

#[test]
fn test_outline_project_rules_are_additive() -> Result<()> {
  let dir = create_test_files([
    (
      "sgconfig.yml",
      "ruleDirs: []\noutlineRules:\n  javascript: extra.yml\ncustomLanguages:\n  unused:\n    libraryPath: missing.so\n    extensions: [unused]\n    outlineRules: missing.yml\n",
    ),
    (
      "extra.yml",
      r#"id: project-call
language: JavaScript
role: item
symbolType: function
rule:
  pattern: console.log($ARG)
name: project-note
"#,
    ),
    (
      "cli.yml",
      r#"id: cli-call
language: JavaScript
role: item
symbolType: function
rule:
  pattern: console.warn($ARG)
name: cli-note
"#,
    ),
    (
      "a.js",
      "function builtin() {}\nconsole.log('note');\nconsole.warn('cli');",
    ),
  ])?;

  // Bundled, project, and explicitly supplied rules are all retained.
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "outline",
      "a.js",
      "--outline-rules",
      "cli.yml",
      "--json=compact",
    ])
    .assert()
    .success()
    .stdout(contains(r#""name":"builtin""#))
    .stdout(contains(r#""name":"project-note""#))
    .stdout(contains(r#""name":"cli-note""#));

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["outline", "a.js", "--json=compact"])
    .assert()
    .success()
    .stdout(contains(r#""name":"builtin""#))
    .stdout(contains(r#""name":"project-note""#));
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "outline",
      "a.js",
      "--no-default-outline-rules",
      "--json=compact",
    ])
    .assert()
    .success()
    .stdout(contains(r#""name":"project-note""#))
    .stdout(contains(r#""name":"builtin""#).not());
  Ok(())
}

#[test]
fn test_outline_project_rules_are_config_relative() -> Result<()> {
  let dir = create_test_files([
    (
      "config/sgconfig.yml",
      "ruleDirs: []\noutlineRules:\n  js: extra.yml\n",
    ),
    (
      "config/extra.yml",
      "id: project-call\nlanguage: JavaScript\nrole: item\nsymbolType: function\nrule:\n  pattern: console.log($ARG)\nname: project-note\n",
    ),
    ("a.js", "console.log('note');"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "--config",
      "config/sgconfig.yml",
      "outline",
      "a.js",
      "--json=compact",
    ])
    .assert()
    .success()
    .stdout(contains(r#""name":"project-note""#));
  Ok(())
}

#[test]
fn test_outline_project_rule_errors() -> Result<()> {
  for (file, message) in [
    ("missing.yml", "Cannot read outline rules"),
    ("invalid.yml", "Cannot parse outline rules"),
  ] {
    let config = format!("ruleDirs: []\noutlineRules:\n  javascript: {file}\n");
    let dir = create_test_files([
      ("sgconfig.yml", config.as_str()),
      ("invalid.yml", "["),
      ("a.js", "console.log('note');"),
    ])?;
    Command::new(cargo_bin!())
      .current_dir(dir.path())
      .args(["outline", "a.js"])
      .assert()
      .failure()
      .stderr(contains(message))
      .stderr(contains(file));
    // Other commands do not read outline rule files.
    Command::new(cargo_bin!())
      .current_dir(dir.path())
      .args(["run", "-p", "console.log($ARG)", "a.js"])
      .assert()
      .success()
      .stdout(contains("console.log('note')"));
  }
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
fn test_config_arg_in_default_run() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", "ruleDirs: []"), ("test.js", "alert(123)")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-c", "sgconfig.yml", "test.js"])
    .assert()
    .success()
    .stdout(contains("alert(123)"));
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
