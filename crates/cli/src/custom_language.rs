use crate::config::find_config_path_with_default;
use crate::utils::ErrorContext as EC;

use anyhow::{Context, Result};
use clap::ValueEnum;
use directories::ProjectDirs;
use std::collections::BTreeSet;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

const TRUST_DIR_ENV: &str = "AST_GREP_TRUST_DIR";

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CustomLanguagePolicy {
  /// Load native custom language libraries for this invocation.
  Allow,
  /// Skip native custom language libraries without reporting an error.
  Ignore,
  /// Load native custom language libraries and remember this project.
  Trust,
  /// Forget this project and ignore its native custom language libraries.
  Revoke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomLanguageAction {
  Load,
  Ignore,
}

pub fn parse_policy(args: &[String]) -> Option<CustomLanguagePolicy> {
  let mut policy = None;
  let mut i = 1;
  while i < args.len() {
    let arg = args[i].as_str();
    if arg == "--" {
      break;
    }
    let value = if arg == "--custom-languages" {
      if policy.is_some() {
        return Some(CustomLanguagePolicy::Ignore);
      }
      // Safe fallback while Clap handles a missing or invalid value.
      policy = Some(CustomLanguagePolicy::Ignore);
      i += 1;
      args.get(i).map(String::as_str)
    } else {
      let value = arg.strip_prefix("--custom-languages=");
      if value.is_some() {
        if policy.is_some() {
          return Some(CustomLanguagePolicy::Ignore);
        }
        policy = Some(CustomLanguagePolicy::Ignore);
      }
      value
    };
    if let Some(value) = value {
      policy = match value {
        "allow" => Some(CustomLanguagePolicy::Allow),
        "ignore" => Some(CustomLanguagePolicy::Ignore),
        "trust" => Some(CustomLanguagePolicy::Trust),
        "revoke" => Some(CustomLanguagePolicy::Revoke),
        _ => Some(CustomLanguagePolicy::Ignore),
      };
    }
    i += 1;
  }
  policy
}

pub fn resolve(
  policy: Option<CustomLanguagePolicy>,
  config_path: Option<&Path>,
) -> Result<CustomLanguageAction> {
  match policy {
    Some(CustomLanguagePolicy::Trust) => {
      eprintln!(
        "WARNING: native custom languages execute code with your permissions. \
ast-grep remembers the configuration path, not its contents."
      );
      confirm_trust(config_path)?;
      set_trusted(config_path, true)?;
      Ok(CustomLanguageAction::Load)
    }
    Some(CustomLanguagePolicy::Revoke) => {
      set_trusted(config_path, false)?;
      Ok(CustomLanguageAction::Ignore)
    }
    None if is_trusted(config_path) => Ok(CustomLanguageAction::Load),
    Some(CustomLanguagePolicy::Allow) => Ok(CustomLanguageAction::Load),
    Some(CustomLanguagePolicy::Ignore) | None => Ok(CustomLanguageAction::Ignore),
  }
}

fn confirm_trust(config_path: Option<&Path>) -> Result<()> {
  if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
    return Err(anyhow::anyhow!(EC::TrustRequiresInteractiveTerminal));
  }
  let config_path = canonical_config_path(config_path)?;
  let prompt = format!(
    "Trust native custom languages configured by {}?",
    config_path.display()
  );
  if !inquire::Confirm::new(&prompt)
    .with_default(false)
    .prompt()?
  {
    anyhow::bail!("Custom language trust was not granted");
  }
  Ok(())
}

fn is_trusted(config_path: Option<&Path>) -> bool {
  canonical_config_path(config_path)
    .and_then(|path| load_store().map(|store| store.contains(&path)))
    .unwrap_or(false)
}

fn set_trusted(config_path: Option<&Path>, trusted: bool) -> Result<()> {
  let config_path = canonical_config_path(config_path)?;
  let mut store = load_store()?;
  let changed = if trusted {
    store.insert(config_path)
  } else {
    store.remove(&config_path)
  };
  if changed {
    save_store(&store)?;
  }
  Ok(())
}

fn load_store() -> Result<BTreeSet<PathBuf>> {
  match fs::read(trust_store_path()?) {
    Ok(json) => serde_json::from_slice(&json).context("Cannot parse the ast-grep trust store"),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
    Err(error) => Err(error).context("Cannot read the ast-grep trust store"),
  }
}

fn save_store(store: &BTreeSet<PathBuf>) -> Result<()> {
  let path = trust_store_path()?;
  let parent = path.parent().expect("trust store path has a parent");
  fs::create_dir_all(parent).context("Cannot create the ast-grep configuration directory")?;
  let mut temp = tempfile::NamedTempFile::new_in(parent)?;
  temp.write_all(&serde_json::to_vec_pretty(store)?)?;
  temp.persist(path).map_err(|error| error.error)?;
  Ok(())
}

fn trust_store_path() -> Result<PathBuf> {
  if let Some(path) = std::env::var_os(TRUST_DIR_ENV) {
    return Ok(PathBuf::from(path).join("trusted-configs.json"));
  }
  let dirs = ProjectDirs::from("", "", "ast-grep")
    .context("Cannot determine the ast-grep configuration directory")?;
  Ok(dirs.config_dir().join("trusted-configs.json"))
}

fn canonical_config_path(config_path: Option<&Path>) -> Result<PathBuf> {
  let config_path = find_config_path_with_default(config_path.map(Path::to_path_buf))?
    .context("Cannot find the ast-grep configuration file")?;
  fs::canonicalize(config_path).context("Cannot resolve the ast-grep configuration path")
}

#[cfg(test)]
mod test {
  use super::*;

  fn parse(args: &[&str]) -> Option<CustomLanguagePolicy> {
    parse_policy(
      &args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>(),
    )
  }

  #[test]
  fn test_parse_policy() {
    assert_eq!(parse(&["sg", "scan"]), None);
    assert_eq!(
      parse(&["sg", "scan", "--custom-languages=allow"]),
      Some(CustomLanguagePolicy::Allow),
    );
    assert_eq!(
      parse(&["sg", "run", "--", "--custom-languages", "allow"]),
      None,
    );
    assert_eq!(
      parse(&["sg", "scan", "--custom-languages", "trust"]),
      Some(CustomLanguagePolicy::Trust),
    );
    assert_eq!(
      parse(&["sg", "scan", "--custom-languages=revoke"]),
      Some(CustomLanguagePolicy::Revoke),
    );
  }
}
