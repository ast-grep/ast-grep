use crate::error::ErrorContext as EC;
use crate::lang::{CustomLang, LanguageGlobs, SgLang};

use anyhow::{Context, Result};
use ast_grep_config::{
  from_str, from_yaml_string, DeserializeEnv, GlobalRules, RuleCollection, RuleConfig,
};
use ast_grep_language::config_file_type;
use ignore::WalkBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestConfig {
  pub test_dir: PathBuf,
  /// Specify the directory containing snapshots. The path is relative to `test_dir`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub snapshot_dir: Option<PathBuf>,
}

impl From<PathBuf> for TestConfig {
  fn from(path: PathBuf) -> Self {
    TestConfig {
      test_dir: path,
      snapshot_dir: None,
    }
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AstGrepConfig {
  /// YAML rule directories
  pub rule_dirs: Vec<PathBuf>,
  /// test configurations
  #[serde(skip_serializing_if = "Option::is_none")]
  pub test_configs: Option<Vec<TestConfig>>,
  /// util rules directories
  #[serde(skip_serializing_if = "Option::is_none")]
  pub util_dirs: Option<Vec<PathBuf>>,
  /// configuration for custom languages
  #[serde(skip_serializing_if = "Option::is_none")]
  pub custom_languages: Option<HashMap<String, CustomLang>>,
  /// additional file globs for languages
  #[serde(skip_serializing_if = "Option::is_none")]
  pub language_globs: Option<LanguageGlobs>,
}

pub fn find_rules(
  config_path: Option<PathBuf>,
  rule_filter: Option<&Regex>,
) -> Result<RuleCollection<SgLang>> {
  let config_path =
    find_config_path_with_default(config_path, None).context(EC::ReadConfiguration)?;
  let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
  let sg_config: AstGrepConfig = from_str(&config_str).context(EC::ParseConfiguration)?;
  let base_dir = config_path
    .parent()
    .expect("config file must have parent directory");
  let global_rules = find_util_rules(base_dir, sg_config.util_dirs)?;
  read_directory_yaml(base_dir, sg_config.rule_dirs, global_rules, rule_filter)
}

pub fn register_custom_language(config_path: Option<PathBuf>) -> Result<()> {
  let Ok(mut path) = find_config_path_with_default(config_path, None) else {
    return Ok(()); // do not report error if no sgconfig.yml is found
  };
  let Ok(config_str) = read_to_string(&path) else {
    return Ok(()); // suppress error when register custom lang
  };
  let sg_config: AstGrepConfig = from_str(&config_str).context(EC::ParseConfiguration)?;
  path.pop();
  if let Some(custom_langs) = sg_config.custom_languages {
    SgLang::register_custom_language(path, custom_langs);
  }
  if let Some(globs) = sg_config.language_globs {
    SgLang::register_globs(globs)?;
  }
  Ok(())
}

fn find_util_rules(
  base_dir: &Path,
  util_dirs: Option<Vec<PathBuf>>,
) -> Result<GlobalRules<SgLang>> {
  let Some(util_dirs) = util_dirs else {
    return Ok(GlobalRules::default());
  };
  let mut utils = vec![];
  // TODO: use WalkBuilder::add to avoid loop
  for dir in util_dirs {
    let dir_path = base_dir.join(dir);
    let walker = WalkBuilder::new(&dir_path)
      .types(config_file_type())
      .build();
    for dir in walker {
      let config_file = dir.with_context(|| EC::WalkRuleDir(dir_path.clone()))?;
      // file_type is None only if it is stdin, safe to unwrap here
      if !config_file
        .file_type()
        .expect("file type should be available for non-stdin")
        .is_file()
      {
        continue;
      }
      let path = config_file.path();
      let file = read_to_string(path)?;
      let new_configs = from_str(&file)?;
      utils.push(new_configs);
    }
  }
  let ret = DeserializeEnv::parse_global_utils(utils).context(EC::InvalidGlobalUtils)?;
  Ok(ret)
}

fn read_directory_yaml(
  base_dir: &Path,
  rule_dirs: Vec<PathBuf>,
  global_rules: GlobalRules<SgLang>,
  rule_filter: Option<&Regex>,
) -> Result<RuleCollection<SgLang>> {
  let mut configs = vec![];
  for dir in rule_dirs {
    let dir_path = base_dir.join(dir);
    let walker = WalkBuilder::new(&dir_path)
      .types(config_file_type())
      .build();
    for dir in walker {
      let config_file = dir.with_context(|| EC::WalkRuleDir(dir_path.clone()))?;
      // file_type is None only if it is stdin, safe to unwrap here
      if !config_file
        .file_type()
        .expect("file type should be available for non-stdin")
        .is_file()
      {
        continue;
      }
      let path = config_file.path();
      let new_configs = read_rule_file(path, Some(&global_rules))?;
      configs.extend(new_configs);
    }
  }

  let configs = if let Some(filter) = rule_filter {
    filter_rule_by_regex(configs, filter)?
  } else {
    configs
  };

  RuleCollection::try_new(configs).context(EC::GlobPattern)
}

fn filter_rule_by_regex(
  configs: Vec<RuleConfig<SgLang>>,
  filter: &Regex,
) -> Result<Vec<RuleConfig<SgLang>>> {
  let selected: Vec<_> = configs
    .into_iter()
    .filter(|c| filter.is_match(&c.id))
    .collect();

  if selected.is_empty() {
    Err(anyhow::anyhow!(EC::RuleNotFound(filter.to_string())))
  } else {
    Ok(selected)
  }
}

pub fn read_rule_file(
  path: &Path,
  global_rules: Option<&GlobalRules<SgLang>>,
) -> Result<Vec<RuleConfig<SgLang>>> {
  let yaml = read_to_string(path).with_context(|| EC::ReadRule(path.to_path_buf()))?;
  let parsed = if let Some(globals) = global_rules {
    from_yaml_string(&yaml, globals)
  } else {
    from_yaml_string(&yaml, &Default::default())
  };
  parsed.with_context(|| EC::ParseRule(path.to_path_buf()))
}

/// Returns the base_directory where config is and config object.
pub fn read_config_from_dir<P: AsRef<Path>>(path: P) -> Result<Option<(PathBuf, AstGrepConfig)>> {
  let mut config_path =
    find_config_path_with_default(None, Some(path.as_ref())).context(EC::ReadConfiguration)?;
  if !config_path.is_file() {
    return Ok(None);
  }
  let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
  let sg_config = from_str(&config_str).context(EC::ParseConfiguration)?;
  // remove sgconfig.yml from the path
  config_path.pop(); // ./sg_config -> ./
  Ok(Some((config_path, sg_config)))
}

const CONFIG_FILE: &str = "sgconfig.yml";

pub fn find_config_path_with_default(
  config_path: Option<PathBuf>,
  base: Option<&Path>,
) -> Result<PathBuf> {
  if let Some(config) = config_path {
    return Ok(config);
  }
  let mut path = if let Some(base) = base {
    base.to_path_buf()
  } else {
    std::env::current_dir()?
  };
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
