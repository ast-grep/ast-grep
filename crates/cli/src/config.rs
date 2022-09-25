use crate::error::ErrorContext;
use crate::languages::{config_file_type, SupportLang};
use anyhow::{Context, Result};
use ast_grep_config::{deserialize_sgconfig, from_yaml_string, RuleCollection};
use ignore::WalkBuilder;
use std::fs::read_to_string;

pub fn find_config(config_path: Option<String>) -> Result<RuleCollection<SupportLang>> {
  let config_path = config_path.unwrap_or_else(find_default_config);
  let config_str = read_to_string(config_path).context(ErrorContext::CannotFindConfiguration)?;
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

fn find_default_config() -> String {
  "sgconfig.yml".to_string()
}
