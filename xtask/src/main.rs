use anyhow::{bail, Context, Result};
use serde_json::{from_str as parse_json, to_string_pretty, Value as JSON};
use std::env::args;
use std::fs::{self, read_dir, read_to_string};
use std::path::Path;
use std::process::{Command, Stdio};
use toml_edit::{value as to_toml, Document};

fn main() -> Result<()> {
  let version = get_new_version()?;
  check_git_status()?;
  bump_version(&version)?;
  commit_and_tag(&version)?;
  update_and_commit_changelog()?;
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
  update_cargo_lock()?;
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
    edit_json(path, version)?;
  }
  Ok(())
}

fn edit_json<P: AsRef<Path>>(path: P, version: &str) -> Result<()> {
  let json_str = read_to_string(&path)?;
  let mut json: JSON = parse_json(&json_str)?;
  json["version"] = version.into();
  fs::write(path, to_string_pretty(&json)?)?;
  Ok(())
}

fn update_napi(version: &str) -> Result<()> {
  let napi_path = "crates/napi/package.json";
  edit_json(napi_path, version)?;
  for entry in read_dir("crates/napi/npm")? {
    let path = entry?.path();
    if !path.is_dir() {
      continue;
    }
    let path = path.join("package.json");
    edit_json(path, version)?;
  }
  Ok(())
}

fn edit_toml<P: AsRef<Path>>(path: P, version: &str) -> Result<()> {
  let mut toml: Document = read_to_string(&path)?.parse()?;
  toml["package"]["version"] = to_toml(version);
  let deps = toml["dependencies"]
    .as_table_mut()
    .context("dep should be table")?;
  for (key, value) in deps.iter_mut() {
    if !key.starts_with("ast-grep-") {
      continue;
    }
    if value.is_str() {
      *value = to_toml(version);
      continue;
    }
    if let Some(inline) = value.as_inline_table_mut() {
      inline["version"] = version.into();
    }
  }
  fs::write(path, toml.to_string())?;
  Ok(())
}

fn update_crates(version: &str) -> Result<()> {
  for entry in read_dir("crates")? {
    let path = entry?.path();
    if !path.is_dir() {
      continue;
    }
    let toml_path = path.join("Cargo.toml");
    edit_toml(toml_path, version)?;
  }
  // update benches
  let toml_path = Path::new("benches/Cargo.toml");
  edit_toml(toml_path, version)?;
  Ok(())
}

fn update_cargo_lock() -> Result<()> {
  if Command::new("cargo").args(["build"]).status()?.success() {
    Ok(())
  } else {
    bail!("cargo build fail! cannot update Cargo.lock")
  }
}

fn commit_and_tag(version: &str) -> Result<()> {
  let commit = Command::new("git")
    .arg("commit")
    .arg("-am")
    .arg(format!("{}\nbump version", version))
    .spawn()?
    .wait()?;
  if !commit.success() {
    bail!("commit failed");
  }
  let tag = Command::new("git")
    .arg("tag")
    .arg(version)
    .spawn()?
    .wait()?;
  if !tag.success() {
    bail!("create tag failed");
  }
  Ok(())
}

fn update_and_commit_changelog() -> Result<()> {
  Command::new("auto-changelog")
    .spawn()
    .context("cannot run command `auto-changelog`. Please install it.")?
    .wait()?;
  Ok(())
}
