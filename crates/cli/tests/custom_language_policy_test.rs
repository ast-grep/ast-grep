mod common;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
use common::create_test_files;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::PathBuf;

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
