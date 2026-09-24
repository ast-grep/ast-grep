mod common;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
use common::create_test_files;
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const CUSTOM_LANGUAGE_CONFIG: &str = r#"
ruleDirs: []
customLanguages:
  example:
    libraryPath: missing-library.so
    extensions: [example]
"#;

fn loadable_custom_language_config() -> Option<String> {
  let library = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    "json-mac.so"
  } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    "json-linux.so"
  } else {
    return None;
  };
  let library = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures")
    .join(library);
  Some(format!(
    "customLanguages:\n  myjson:\n    libraryPath: {}\n    languageSymbol: tree_sitter_json\n    extensions: [myjson]\n",
    library.display()
  ))
}

#[test]
fn custom_languages_require_explicit_opt_in() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .arg("scan")
    .assert()
    .failure()
    .stderr(contains(
      "Custom language libraries require explicit opt-in",
    ));
  Ok(())
}

#[test]
fn allow_custom_languages_attempts_to_load_the_library() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--allow-custom-languages"])
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library"));
  Ok(())
}

#[test]
fn positional_path_does_not_enable_custom_languages() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "foo", "--", "--allow-custom-languages"])
    .assert()
    .failure()
    .stderr(contains(
      "Custom language libraries require explicit opt-in",
    ));
  Ok(())
}

#[test]
fn denied_custom_languages_do_not_overwrite_project_config() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["new", "project", "--yes"])
    .assert()
    .failure()
    .stderr(contains(
      "Custom language libraries require explicit opt-in",
    ));
  assert_eq!(
    fs::read_to_string(dir.path().join("sgconfig.yml"))?,
    CUSTOM_LANGUAGE_CONFIG
  );
  Ok(())
}

#[test]
fn allowed_custom_language_is_registered_before_cli_parsing() -> Result<()> {
  let Some(config) = loadable_custom_language_config() else {
    return Ok(());
  };
  let dir = create_test_files([("sgconfig.yml", config.as_str())])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "run",
      "-p",
      "1",
      "-l",
      "myjson",
      "--allow-custom-languages",
      "--stdin",
    ])
    .write_stdin(r#"{"key": 1}"#)
    .assert()
    .success();
  Ok(())
}

#[test]
fn persistent_trust_is_path_based_and_can_be_revoked() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG),
    ("missing-library.so", "not really a native library"),
  ])?;
  let trust_dir = TempDir::new()?;
  let command = || {
    let mut command = Command::new(cargo_bin!());
    command
      .current_dir(dir.path())
      .env("AST_GREP_TRUST_DIR", trust_dir.path());
    command
  };

  command()
    .args(["trust", "-y"])
    .assert()
    .success()
    .stdout(contains("Trusted"));

  // A trusted project proceeds to the loader (which rejects the test fixture).
  command()
    .arg("scan")
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library"));

  fs::write(
    dir.path().join("sgconfig.yml"),
    format!("{CUSTOM_LANGUAGE_CONFIG}\n# changed"),
  )?;
  fs::write(
    dir.path().join("missing-library.so"),
    "changed native library contents",
  )?;
  command()
    .arg("scan")
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library"));

  command()
    .args(["trust", "--revoke"])
    .assert()
    .success()
    .stdout(contains("Revoked trust"));
  command().arg("scan").assert().failure().stderr(contains(
    "Custom language libraries require explicit opt-in",
  ));
  Ok(())
}

#[test]
fn trust_requires_yes_without_a_terminal() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG),
    ("missing-library.so", "not really a native library"),
  ])?;
  let trust_dir = TempDir::new()?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .env("AST_GREP_TRUST_DIR", trust_dir.path())
    .arg("trust")
    .assert()
    .failure()
    .stderr(contains("Pass `-y`"));

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .env("AST_GREP_TRUST_DIR", trust_dir.path())
    .arg("scan")
    .assert()
    .failure()
    .stderr(contains(
      "Custom language libraries require explicit opt-in",
    ));
  Ok(())
}
