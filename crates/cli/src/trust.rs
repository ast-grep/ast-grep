use crate::config::find_config_path_with_default;
use crate::utils::ErrorContext as EC;

use anyhow::{Context, Result};
use clap::Args;
use directories::BaseDirs;
use inquire::Confirm;
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const TRUST_STORE_VERSION: u8 = 1;
const TRUST_DIR_ENV: &str = "AST_GREP_TRUST_DIR";
const TRUST_WARNING: &str = "WARNING: TRUSTING NATIVE CODE\n\
Native custom language libraries execute code with your user permissions.\n\
ast-grep trusts this configuration path, not its current contents. Anyone who can later modify\n\
the configuration or a referenced library can cause ast-grep to execute different, potentially\n\
malicious native code.";

#[derive(Args)]
pub struct TrustArg {
  /// Trust the project without asking for confirmation.
  #[arg(short = 'y', long)]
  yes: bool,
  /// Revoke trust for the project.
  #[arg(long)]
  revoke: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustStore {
  version: u8,
  trusted_configs: BTreeSet<PathBuf>,
}

pub fn run_trust(arg: TrustArg, config_path: Option<PathBuf>) -> Result<ExitCode> {
  let config_path = if arg.revoke {
    canonical_config_path_for_revoke(config_path)?
  } else {
    canonical_config_path(config_path)?
  };
  let mut store = load_store()?;
  if arg.revoke {
    if store.trusted_configs.remove(&config_path) {
      save_store(&store)?;
      println!("Revoked trust for {}", config_path.display());
    } else {
      println!("Configuration is not trusted: {}", config_path.display());
    }
    return Ok(ExitCode::SUCCESS);
  }

  if store.trusted_configs.contains(&config_path) {
    println!(
      "Configuration is already trusted: {}",
      config_path.display()
    );
    return Ok(ExitCode::SUCCESS);
  }

  eprintln!("{TRUST_WARNING}");
  if !arg.yes && !std::io::stdin().is_terminal() {
    return Err(anyhow::anyhow!(EC::TrustConfirmationRequired));
  }
  let confirmed = arg.yes
    || Confirm::new(&format!(
      "Trust {} and allow it to load native custom language libraries?",
      config_path.display()
    ))
    .with_default(false)
    .prompt()?;
  if !confirmed {
    println!("Trust was not granted.");
    return Ok(ExitCode::SUCCESS);
  }

  store.version = TRUST_STORE_VERSION;
  store.trusted_configs.insert(config_path.clone());
  save_store(&store)?;
  println!("Trusted {}", config_path.display());
  Ok(ExitCode::SUCCESS)
}

pub fn is_config_trusted(config_path: Option<&Path>) -> bool {
  let Ok(config_path) = canonical_config_path(config_path.map(Path::to_path_buf)) else {
    return false;
  };
  load_store().is_ok_and(|store| {
    store.version == TRUST_STORE_VERSION && store.trusted_configs.contains(&config_path)
  })
}

fn load_store() -> Result<TrustStore> {
  let path = trust_store_path()?;
  match fs::read(path) {
    Ok(serialized) => {
      serde_json::from_slice(&serialized).context("Cannot parse the ast-grep trust store")
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(TrustStore::default()),
    Err(error) => Err(error).context("Cannot read the ast-grep trust store"),
  }
}

fn save_store(store: &TrustStore) -> Result<()> {
  let path = trust_store_path()?;
  let parent = path
    .parent()
    .expect("trust store path must have a parent directory");
  fs::create_dir_all(parent).context("Cannot create the ast-grep configuration directory")?;
  let serialized = serde_json::to_vec_pretty(store)?;
  let mut temp = tempfile::NamedTempFile::new_in(parent)
    .context("Cannot create a temporary ast-grep trust store")?;
  temp
    .write_all(&serialized)
    .context("Cannot write the temporary ast-grep trust store")?;
  temp
    .as_file()
    .sync_all()
    .context("Cannot flush the temporary ast-grep trust store")?;
  temp
    .persist(&path)
    .map_err(|error| error.error)
    .context("Cannot replace the ast-grep trust store")?;
  Ok(())
}

fn trust_store_path() -> Result<PathBuf> {
  if let Some(path) = std::env::var_os(TRUST_DIR_ENV) {
    return Ok(PathBuf::from(path).join("trusted-configs.json"));
  }
  let dirs = BaseDirs::new().context("Cannot determine the user configuration directory")?;
  Ok(
    dirs
      .config_dir()
      .join("ast-grep")
      .join("trusted-configs.json"),
  )
}

fn canonical_config_path(config_path: Option<PathBuf>) -> Result<PathBuf> {
  let config_path = find_config_path_with_default(config_path)?
    .context("Cannot find the ast-grep configuration file")?;
  fs::canonicalize(config_path).context("Cannot resolve the ast-grep configuration path")
}

fn canonical_config_path_for_revoke(config_path: Option<PathBuf>) -> Result<PathBuf> {
  let Some(config_path) = config_path else {
    return canonical_config_path(None);
  };
  match fs::canonicalize(&config_path) {
    Ok(path) => Ok(path),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      canonicalize_missing_file(&config_path)
    }
    Err(error) => Err(error).context("Cannot resolve the ast-grep configuration path"),
  }
}

fn canonicalize_missing_file(path: &Path) -> Result<PathBuf> {
  let file_name = path
    .file_name()
    .context("The ast-grep configuration path must name a file")?;
  let parent = path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or_else(|| Path::new("."));
  let parent = fs::canonicalize(parent)
    .context("Cannot resolve the ast-grep configuration parent directory")?;
  Ok(parent.join(file_name))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_store_is_empty() {
    let store = TrustStore::default();
    assert_eq!(store.version, 0);
    assert!(store.trusted_configs.is_empty());
  }
}
