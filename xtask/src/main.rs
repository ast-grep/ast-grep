use anyhow::{bail, Context, Result};
use serde_json::{from_str as parse_json, to_string_pretty, Value as JSON};
use std::env::args;
use std::fs::{self, read_dir, read_to_string};
use std::path::Path;
use std::process::{Command, Stdio};

fn main() -> Result<()> {
  let version = get_new_version()?;
  check_git_status()?;
  bump_version(&version)?;
  commit_and_tag(&version)?;
  update_and_commit_changelog(&version)?;
  Ok(())
}

fn get_new_version() -> Result<String> {
  let message = "Version number is missing. Example usage: cargo xtask 0.1.3";
  args().skip(1).next().context(message)
}

fn check_git_status() -> Result<()> {
  let git = Command::new("git")
    .arg("status")
    .arg("--porcelain")
    .stdout(Stdio::piped())
    .spawn()?
    .wait_with_output()?;
  if git.stdout.len() > 0 {
    bail!("The git working directory has uncommitted changes. Please commit or abandon them before release!")
  } else {
    Ok(())
  }
}

fn bump_version(version: &str) -> Result<()> {
  update_npm(&version)?;
  update_napi(&version)?;
  update_crates(&version)?;
  Ok(())
}

fn update_npm(version: &str) -> Result<()> {
  let npm_path = "npm/package.json";
  let root_json = read_to_string(npm_path)?;
  let mut root_json: JSON = parse_json(&root_json)?;
  root_json["version"] = version.into();
  let deps = root_json["optionalDependencies"]
    .as_object_mut()
    .context("parse json error")?;
  for val in deps.values_mut() {
    *val = version.into();
  }
  fs::write(npm_path, to_string_pretty(&root_json)?)?;
  for entry in read_dir("npm/platforms")? {
    let path = entry?.path();
    if !path.is_dir() {
      continue;
    }
    let path = path.join("package.json");
    update_json_version(path, version)?;
  }
  Ok(())
}

fn update_json_version<P: AsRef<Path>>(path: P, version: &str) -> Result<()> {
  let json_str = read_to_string(&path)?;
  let mut json: JSON = parse_json(&json_str)?;
  json["version"] = version.into();
  fs::write(path, to_string_pretty(&json)?)?;
  Ok(())
}

fn update_napi(version: &str) -> Result<()> {
  let napi_path = "crates/napi/package.json";
  update_json_version(napi_path, version)?;
  for entry in read_dir("crates/napi/npm")? {
    let path = entry?.path();
    if !path.is_dir() {
      continue;
    }
    let path = path.join("package.json");
    update_json_version(path, version)?;
  }
  Ok(())
}

fn update_crates(version: &str) -> Result<()> {
  Ok(())
}

fn commit_and_tag(version: &str) -> Result<()> {
  Ok(())
}

fn update_and_commit_changelog(version: &str) -> Result<()> {
  Ok(())
}
