use anyhow::{bail, Result};
use std::process::{Command, Stdio};

fn main() -> Result<()> {
  check_git_status()?;
  update_npm()?;
  update_napi()?;
  update_crates()?;
  Ok(())
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

fn update_npm() -> Result<()> {
  Ok(())
}

fn update_napi() -> Result<()> {
  Ok(())
}

fn update_crates() -> Result<()> {
  Ok(())
}
