use crate::error::ErrorContext;
use crate::languages::{config_file_type, SupportLang};
use anyhow::{Context, Result};
use ast_grep_config::{deserialize_sgconfig, from_yaml_string, RuleCollection};
use ignore::WalkBuilder;
use std::fs::read_to_string;
use std::path::PathBuf;

pub fn find_config(config_path: Option<String>) -> Result<RuleCollection<SupportLang>> {
  let config_path =
    find_config_path_with_default(config_path).context(ErrorContext::CannotReadConfiguration)?;
  let config_str = read_to_string(config_path).context(ErrorContext::CannotReadConfiguration)?;
  let sg_config =
    deserialize_sgconfig(&config_str).context(ErrorContext::CannotParseConfiguration)?;
  let mut configs = vec![];
  for dir in sg_config.rule_dirs {
    let walker = WalkBuilder::new(&dir).types(config_file_type()).build();
    for dir in walker {
      let config_file = dir.unwrap();
      if !config_file.file_type().unwrap().is_file() {
        continue;
      }
      let path = config_file.path();

      let yaml = read_to_string(path).unwrap();
      configs.extend(from_yaml_string(&yaml).unwrap());
    }
  }
  Ok(RuleCollection::new(configs))
}

const CONFIG_FILE: &str = "sgconfig.yml";

fn find_config_path_with_default(config_path: Option<String>) -> Result<PathBuf> {
  if let Some(config) = config_path {
    return Ok(PathBuf::from(config));
  }
  let mut path = std::env::current_dir()?;
  loop {
    let maybe_config = path.join(CONFIG_FILE);
    if maybe_config.exists() {
      break Ok(maybe_config);
    }
    if let Some(parent) = path.parent() {
      path = parent.to_path_buf();
    } else {
      break Ok(PathBuf::from(CONFIG_FILE));
    }
  }
}
