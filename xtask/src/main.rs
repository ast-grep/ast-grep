mod schema;
use anyhow::{bail, Context, Result};
use serde_json::{from_str as parse_json, to_string_pretty, Value as JSON};
use std::env::args;
use std::fs::{self, read_dir, read_to_string};
use std::path::Path;
use std::process::{Command, Stdio};
use toml_edit::{value as to_toml, DocumentMut};

enum Task {
  Schema,
  Release(String),
}

fn get_task() -> Result<Task> {
  let message = "argument is missing. Example usage: \ncargo xtask 0.1.3\ncargo xtask schema";
  let arg = args().nth(1).context(message)?;
  if arg == "schema" {
    Ok(Task::Schema)
  } else {
    Ok(Task::Release(arg))
  }
}

fn main() -> Result<()> {
  match get_task()? {
    Task::Schema => schema::generate_schema(),
    Task::Release(version) => release_new_version(&version),
  }
}

fn release_new_version(version: &str) -> Result<()> {
  check_git_status()?;
  schema::generate_schema()?;
  bump_version(version)?;
  update_and_commit_changelog()?;
  commit_and_tag(version)?;
  Ok(())
}

fn check_git_status() -> Result<()> {
  let git = Command::new("git")
    .arg("status")
    .arg("--porcelain")
    .stdout(Stdio::piped())
    .spawn()?
    .wait_with_output()?;
  if !git.stdout.is_empty() {
    bail!("The git working directory has uncommitted changes. Please commit or abandon them before release!")
  } else {
    Ok(())
  }
}

fn bump_version(version: &str) -> Result<()> {
  update_npm(version)?;
  update_napi(version)?;
  update_python(version)?;
  update_crates(version)?;
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

fn edit_root_toml<P: AsRef<Path>>(path: P, version: &str) -> Result<()> {
  let mut toml: DocumentMut = read_to_string(&path)?.parse()?;
  toml["workspace"]["package"]["version"] = to_toml(version);
  let deps = toml["workspace"]["dependencies"]
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
  // update root toml
  let root_toml = Path::new("Cargo.toml");
  edit_root_toml(root_toml, version)?;
  // no need to update crates or benches
  Ok(())
}

fn update_python(version: &str) -> Result<()> {
  // update pypi pyproject.toml and pyo3 bindings
  for path in ["pyproject.toml", "crates/pyo3/pyproject.toml"] {
    let pyproject = Path::new(path);
    let mut toml: DocumentMut = read_to_string(pyproject)?.parse()?;
    toml["project"]["version"] = to_toml(version);
    fs::write(pyproject, toml.to_string())?;
  }
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
  // NB: napi needs line break to decide npm tag
  // https://github.com/ast-grep/ast-grep/blob/998691d36b477766be92f1ede3c0bc153d0cca42/.github/workflows/napi.yml#L164
  let message = format!("{version}\nbump version");
  let commit = Command::new("git")
    .arg("commit")
    .arg("-am")
    .arg(message)
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
    .arg("-p")
    .arg("npm/package.json")
    .arg("--breaking-pattern")
    .arg("BREAKING CHANGE")
    .spawn()
    .context("cannot run command `auto-changelog`. Please install it.")?
    .wait()?;
  Ok(())
}
