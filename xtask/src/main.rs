use anyhow::{bail, Context, Result};
use std::env::args;
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
  Ok(())
}

fn update_napi(version: &str) -> Result<()> {
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
