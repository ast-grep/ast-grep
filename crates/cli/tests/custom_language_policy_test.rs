mod common;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
use common::create_test_files;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::collections::BTreeSet;
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

fn command_with_trust_store(project: &TempDir, trust_store: &TempDir) -> Command {
  let mut command = Command::new(cargo_bin!());
  command
    .current_dir(project.path())
    .env("AST_GREP_TRUST_DIR", trust_store.path());
  command
}

fn remember_project(config_path: PathBuf, trust_store: &TempDir) -> Result<()> {
  let trusted = BTreeSet::from([config_path.canonicalize()?]);
  fs::write(
    trust_store.path().join("trusted-configs.json"),
    serde_json::to_vec(&trusted)?,
  )?;
  Ok(())
}

#[test]
fn custom_languages_are_ignored_by_default() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .arg("scan")
    .assert()
    .success()
    .stderr(contains("custom languages are ignored"));
  Ok(())
}

#[test]
fn explicit_ignore_skips_custom_languages() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--custom-languages", "ignore"])
    .assert()
    .success()
    .stderr(contains("custom languages are ignored"));
  Ok(())
}

#[test]
fn allow_custom_languages_attempts_to_load_the_library() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;

  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--custom-languages", "allow"])
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
    .args(["-p", "foo", "--", "--custom-languages", "allow"])
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library").not());
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
      "--custom-languages",
      "allow",
      "--stdin",
    ])
    .write_stdin(r#"{"key": 1}"#)
    .assert()
    .success();
  Ok(())
}

#[test]
fn trust_requires_an_interactive_terminal() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;
  let trust_dir = TempDir::new()?;

  command_with_trust_store(&dir, &trust_dir)
    .args(["scan", "--custom-languages", "trust"])
    .assert()
    .failure()
    .stderr(contains("WARNING: native custom languages execute code"))
    .stderr(contains("Cannot trust custom languages non-interactively"))
    .stderr(contains("--custom-languages allow"));
  command_with_trust_store(&dir, &trust_dir)
    .arg("scan")
    .assert()
    .success()
    .stderr(contains("custom languages are ignored"));
  Ok(())
}

#[test]
fn remembered_trust_can_be_overridden_and_revoked() -> Result<()> {
  let dir = create_test_files([("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG)])?;
  let trust_dir = TempDir::new()?;
  remember_project(dir.path().join("sgconfig.yml"), &trust_dir)?;

  command_with_trust_store(&dir, &trust_dir)
    .arg("scan")
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library"));

  // Explicit policies override remembered trust.
  command_with_trust_store(&dir, &trust_dir)
    .args(["scan", "--custom-languages", "ignore"])
    .assert()
    .success();
  command_with_trust_store(&dir, &trust_dir)
    .args(["scan", "--custom-languages", "revoke"])
    .assert()
    .success()
    .stderr(contains("custom languages are ignored"));
  command_with_trust_store(&dir, &trust_dir)
    .arg("scan")
    .assert()
    .success();
  Ok(())
}

#[test]
fn remembered_trust_uses_attached_config_path() -> Result<()> {
  let dir = create_test_files([("project.yml", CUSTOM_LANGUAGE_CONFIG)])?;
  let trust_dir = TempDir::new()?;
  remember_project(dir.path().join("project.yml"), &trust_dir)?;

  command_with_trust_store(&dir, &trust_dir)
    .args(["scan", "-cproject.yml"])
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library"));
  Ok(())
}

#[test]
fn config_path_after_delimiter_is_not_used_for_project_setup() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CUSTOM_LANGUAGE_CONFIG),
    ("other.yml", CUSTOM_LANGUAGE_CONFIG),
  ])?;
  let trust_dir = TempDir::new()?;
  remember_project(dir.path().join("other.yml"), &trust_dir)?;

  command_with_trust_store(&dir, &trust_dir)
    .args(["-p", "foo", "--", "-cother.yml"])
    .assert()
    .failure()
    .stderr(contains("Cannot load custom language library").not());
  Ok(())
}

#[test]
fn deleted_config_can_be_revoked() -> Result<()> {
  let dir = create_test_files([("project.yml", CUSTOM_LANGUAGE_CONFIG)])?;
  let trust_dir = TempDir::new()?;
  let config = dir.path().join("project.yml");
  remember_project(config.clone(), &trust_dir)?;
  fs::remove_file(&config)?;

  command_with_trust_store(&dir, &trust_dir)
    .args(["scan", "--custom-languages", "revoke", "-cproject.yml"])
    .assert()
    .failure();

  let stored: BTreeSet<PathBuf> =
    serde_json::from_slice(&fs::read(trust_dir.path().join("trusted-configs.json"))?)?;
  assert!(stored.is_empty());
  Ok(())
}
