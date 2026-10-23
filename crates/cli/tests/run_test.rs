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

/// Verifies that outline honors project-configured hidden-file searches.
#[test]
fn test_outline_no_ignore_from_config() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", "ruleDirs: []\nnoIgnore:\n  - hidden\n"),
    (".hidden.js", "export function hiddenFunction() {}"),
  ])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["outline", ".", "--json=compact"])
    .assert()
    .success()
    .stdout(contains(r#""name":"hiddenFunction""#));
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

/// Verifies that run honors project-configured hidden-file searches.
#[test]
fn test_no_ignore_from_config() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", "ruleDirs: []\nnoIgnore:\n  - hidden\n"),
    (".hidden.js", "alert(123)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-l", "js"])
    .assert()
    .success()
    .stdout(contains("alert(123)"));
  Ok(())
}

#[test]
fn test_cli_no_ignore_overrides_config() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", "ruleDirs: []\nnoIgnore: [hidden]\n"),
    (".hidden/a.js", "alert(123)"),
    ("visible.js", "alert(456)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "alert($A)", "-l", "js", "--no-ignore", "vcs"])
    .assert()
    .success()
    .stdout(contains("alert(456)"))
    .stdout(contains("alert(123)").not());
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

#[test]
fn test_invalid_utf8_files_are_reported() -> Result<()> {
  let dir = create_test_files([
    ("valid.rs", "fn main() { let x = 1; }"),
    ("empty.rs", ""),
    ("nonmatching.rs", "fn main() {}"),
  ])?;
  let invalid = b"fn main() { let x = 1; } // \xff\xfe\n";
  for i in 0..8 {
    std::fs::write(dir.path().join(format!("invalid-{i}.rs")), invalid)?;
  }
  std::fs::write(
    dir.path().join("large.rs"),
    "// oversized source file\n".repeat(200_001),
  )?;

  for infer_language in [false, true] {
    for output_mode in [None, Some("--json=compact"), Some("--json=stream")] {
      let mut command = Command::new(cargo_bin!());
      command
        .current_dir(dir.path())
        .args(["run", "-p", "let $V = $E;", "--threads=4"]);
      if !infer_language {
        command.args(["-l", "rust"]);
      }
      if let Some(mode) = output_mode {
        command.arg(mode);
      }
      let assertion = command.assert().success();
      let output = assertion.get_output();
      let stderr = String::from_utf8_lossy(&output.stderr);
      assert_eq!(stderr.lines().count(), 8, "{stderr}");
      for i in 0..8 {
        let name = format!("invalid-{i}.rs");
        let lines: Vec<_> = stderr.lines().filter(|line| line.contains(&name)).collect();
        assert_eq!(lines.len(), 1, "{stderr}");
        assert!(lines[0].contains("not valid UTF-8, skipping"));
        assert_eq!(std::fs::read(dir.path().join(name))?, invalid);
      }
      if output_mode == Some("--json=compact") {
        let matches: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(matches.as_array().unwrap().len(), 1);
        assert_eq!(matches[0]["file"], "valid.rs");
      } else if output_mode == Some("--json=stream") {
        let matches: Vec<serde_json::Value> = serde_json::Deserializer::from_slice(&output.stdout)
          .into_iter()
          .collect::<std::result::Result<_, _>>()?;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["file"], "valid.rs");
      } else {
        assert!(String::from_utf8_lossy(&output.stdout).contains("valid.rs"));
      }
    }
  }
  Ok(())
}

#[test]
fn test_invalid_utf8_files_preserve_no_match_exit_status() -> Result<()> {
  let dir = create_test_files([("empty.rs", ""), ("nonmatching.rs", "fn main() {}")])?;
  std::fs::write(dir.path().join("invalid.rs"), b"let x = 1; // \xff\n")?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["run", "-p", "let $V = $E;", "-l", "rust", "--json=compact"])
    .assert()
    .code(1)
    .stdout("[]\n")
    .stderr(contains("invalid.rs: not valid UTF-8, skipping"));
  Ok(())
}

#[test]
fn test_outline_invalid_utf8_preserves_contextual_error() -> Result<()> {
  let dir = create_test_files([])?;
  std::fs::write(dir.path().join("invalid.rs"), b"fn main() {} // \xff\n")?;
  for output_mode in [None, Some("--json=compact"), Some("--json=stream")] {
    let mut command = Command::new(cargo_bin!());
    command
      .current_dir(dir.path())
      .args(["outline", "-l", "rust", "invalid.rs"]);
    if let Some(mode) = output_mode {
      command.arg(mode);
    }
    let assertion = command.assert().success();
    let output = assertion.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
      stderr
        .starts_with("ERROR: Cannot extract outline from invalid.rs: Cannot read file invalid.rs:")
    );
    assert_eq!(stderr.matches("ERROR:").count(), 1, "{stderr}");
    assert!(!stderr.contains("Warning:"), "{stderr}");
    if output_mode == Some("--json=compact") {
      let files: serde_json::Value = serde_json::from_slice(&output.stdout)?;
      assert_eq!(files, serde_json::json!([]));
    }
  }
  Ok(())
}
