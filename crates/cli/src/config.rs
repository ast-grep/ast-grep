use crate::lang::{CustomLang, LanguageGlobs, SerializableInjection, SgLang};
use crate::utils::{ErrorContext as EC, NoIgnore, RuleOverwrite, RuleTrace};

use anyhow::{Context, Result};
use ast_grep_config::{
  DeserializeEnv, GlobalRules, RuleCollection, RuleConfig, from_str, from_yaml_string,
};
use ast_grep_language::config_file_type;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
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
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
  /// injection config for embedded languages
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub language_injections: Vec<SerializableInjection>,
}

#[derive(Clone)]
pub struct ProjectConfig {
  pub project_dir: PathBuf,
  /// YAML rule directories
  pub rule_dirs: Vec<PathBuf>,
  /// YAML outline rule files configured by custom languages
  pub outline_rules: Vec<PathBuf>,
  /// test configurations
  pub test_configs: Option<Vec<TestConfig>>,
  /// util rules directories
  pub util_dirs: Option<Vec<PathBuf>>,
}

impl ProjectConfig {
  // return None if config file does not exist
  fn discover_project(config_path: Option<PathBuf>) -> Result<Option<(PathBuf, AstGrepConfig)>> {
    let config_path = find_config_path_with_default(config_path).context(EC::ProjectNotExist)?;
    // NOTE: if config file does not exist, return None
    let Some(config_path) = config_path else {
      return Ok(None);
    };
    let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
    let sg_config: AstGrepConfig = from_str(&config_str).context(EC::ParseConfiguration)?;
    let project_dir = config_path
      .parent()
      .expect("config file must have parent directory")
      .to_path_buf();
    Ok(Some((project_dir, sg_config)))
  }

  pub fn find_rules(
    &self,
    rule_overwrite: RuleOverwrite,
  ) -> Result<(RuleCollection<SgLang>, RuleTrace)> {
    self.find_rules_with_no_ignore(rule_overwrite, &NoIgnore::default())
  }

  pub fn find_rules_with_no_ignore(
    &self,
    rule_overwrite: RuleOverwrite,
    no_ignore: &NoIgnore,
  ) -> Result<(RuleCollection<SgLang>, RuleTrace)> {
    let global_rules = find_util_rules(self, no_ignore)?;
    read_directory_yaml(self, global_rules, rule_overwrite, no_ignore)
  }

  /// returns a Result of Result.
  /// The inner Result is for configuration not found, or ProjectNotExist
  /// The outer Result is for definitely wrong config.
  pub fn setup(config_path: Option<PathBuf>) -> Result<Result<Self>> {
    let Some((project_dir, mut sg_config)) = Self::discover_project(config_path)? else {
      return Ok(Err(anyhow::anyhow!(EC::ProjectNotExist)));
    };
    let outline_rules =
      custom_language_outline_rules(&project_dir, sg_config.custom_languages.as_ref());
    let config = ProjectConfig {
      project_dir,
      rule_dirs: sg_config.rule_dirs.drain(..).collect(),
      outline_rules,
      test_configs: sg_config.test_configs.take(),
      util_dirs: sg_config.util_dirs.take(),
    };
    // sg_config will not use rule dirs and test configs anymore
    register_custom_language(&config.project_dir, sg_config)?;
    Ok(Ok(config))
  }
}

fn custom_language_outline_rules(
  project_dir: &Path,
  custom_languages: Option<&HashMap<String, CustomLang>>,
) -> Vec<PathBuf> {
  custom_languages
    .into_iter()
    .flat_map(HashMap::values)
    .filter_map(|lang| lang.outline_rules.as_ref())
    .map(|path| project_dir.join(path))
    .collect()
}

fn register_custom_language(project_dir: &Path, sg_config: AstGrepConfig) -> Result<()> {
  if let Some(custom_langs) = sg_config.custom_languages {
    SgLang::register_custom_language(project_dir, custom_langs)?;
  }
  if let Some(globs) = sg_config.language_globs {
    SgLang::register_globs(globs)?;
  }
  SgLang::register_injections(sg_config.language_injections)?;
  Ok(())
}

fn build_util_walker(
  base_dir: &Path,
  util_dirs: &Option<Vec<PathBuf>>,
  no_ignore: &NoIgnore,
) -> Option<WalkBuilder> {
  let mut util_dirs = util_dirs.as_ref()?.iter();
  let first = util_dirs.next()?;
  let mut walker = no_ignore.walk(&[base_dir.join(first)]);
  for dir in util_dirs {
    walker.add(base_dir.join(dir));
  }
  Some(walker)
}

fn find_util_rules(config: &ProjectConfig, no_ignore: &NoIgnore) -> Result<GlobalRules> {
  let ProjectConfig {
    project_dir,
    util_dirs,
    ..
  } = config;
  let Some(mut walker) = build_util_walker(project_dir, util_dirs, no_ignore) else {
    return Ok(GlobalRules::default());
  };
  let mut utils = vec![];
  let walker = walker.types(config_file_type()).build();
  for dir in walker {
    let config_file = dir.with_context(|| EC::WalkRuleDir(PathBuf::new()))?;
    // file_type is None only if it is stdin, safe to panic here
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

  let ret = DeserializeEnv::<SgLang>::parse_global_utils(utils).context(EC::InvalidGlobalUtils)?;
  Ok(ret)
}

fn read_directory_yaml(
  config: &ProjectConfig,
  global_rules: GlobalRules,
  rule_overwrite: RuleOverwrite,
  no_ignore: &NoIgnore,
) -> Result<(RuleCollection<SgLang>, RuleTrace)> {
  let mut configs = vec![];
  let ProjectConfig {
    project_dir,
    rule_dirs,
    ..
  } = config;
  for dir in rule_dirs {
    let dir_path = project_dir.join(dir);
    let walker = no_ignore
      .walk(&[dir_path.clone()])
      .types(config_file_type())
      .build();
    for dir in walker {
      let config_file = dir.with_context(|| EC::WalkRuleDir(dir_path.clone()))?;
      // file_type is None only if it is stdin, safe to panic here
      if !config_file
        .file_type()
        .expect("file type should be available for non-stdin")
        .is_file()
      {
        continue;
      }
      let path = config_file.path();
      let new_configs = read_rule_file(path, &global_rules)?;
      configs.extend(new_configs);
    }
  }
  if let Some(duplicated_id) = configs
    .iter()
    .filter(|c| !c.id.is_empty())
    .try_fold(HashSet::new(), |mut seen, c| {
      if seen.insert(&c.id) {
        Ok(seen)
      } else {
        Err(&c.id)
      }
    })
    .err()
  {
    return Err(anyhow::anyhow!(EC::DuplicateRuleId(duplicated_id.into())));
  }
  let total_rule_count = configs.len();

  let configs = rule_overwrite.process_configs(configs)?;
  let collection = RuleCollection::try_new(configs).context(EC::GlobPattern)?;
  let effective_rule_count = collection.total_rule_count();
  let trace = RuleTrace {
    file_trace: Default::default(),
    effective_rule_count,
    skipped_rule_count: total_rule_count - effective_rule_count,
  };
  Ok((collection, trace))
}

pub fn with_rule_stats(
  configs: Vec<RuleConfig<SgLang>>,
) -> Result<(RuleCollection<SgLang>, RuleTrace)> {
  let total_rule_count = configs.len();
  let collection = RuleCollection::try_new(configs).context(EC::GlobPattern)?;
  let effective_rule_count = collection.total_rule_count();
  let trace = RuleTrace {
    file_trace: Default::default(),
    effective_rule_count,
    skipped_rule_count: total_rule_count - effective_rule_count,
  };
  Ok((collection, trace))
}

pub fn read_rule_file(path: &Path, global_rules: &GlobalRules) -> Result<Vec<RuleConfig<SgLang>>> {
  let yaml = read_to_string(path).with_context(|| EC::ReadRule(path.to_path_buf()))?;
  let parsed = from_yaml_string(&yaml, global_rules);
  let mut rules = parsed.with_context(|| EC::ParseRule(path.to_path_buf()))?;
  let default_id = path.file_stem().and_then(|s| s.to_str());
  let has_multiple = rules.len() > 1;
  for (i, rule) in rules.iter_mut().enumerate() {
    if rule.id.is_empty() {
      let id = default_id.ok_or_else(|| anyhow::anyhow!(EC::InvalidRuleId(path.to_path_buf())))?;
      rule.id = if has_multiple {
        format!("{id}-{i}")
      } else {
        id.into()
      };
    }
  }
  Ok(rules)
}

const CONFIG_FILE_YML: &str = "sgconfig.yml";
const CONFIG_FILE_YAML: &str = "sgconfig.yaml";

/// return None if config file does not exist
fn find_config_path_with_default(config_path: Option<PathBuf>) -> Result<Option<PathBuf>> {
  if config_path.is_some() {
    return Ok(config_path);
  }
  let mut path = std::env::current_dir()?;
  loop {
    let yml = path.join(CONFIG_FILE_YML);
    let yaml = path.join(CONFIG_FILE_YAML);
    if yml.exists() {
      break Ok(Some(yml));
    } else if yaml.exists() {
      break Ok(Some(yaml));
    }
    if let Some(parent) = path.parent() {
      path = parent.to_path_buf();
    } else {
      break Ok(None);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn custom_language_outline_rules_are_project_relative() {
    let config: AstGrepConfig = from_str(
      r#"
customLanguages:
  blade:
    libraryPath: parsers/blade.so
    extensions: [blade.php]
    outlineRules: outline/blade.yml
"#,
    )
    .expect("config should parse");

    let paths =
      custom_language_outline_rules(Path::new("/project"), config.custom_languages.as_ref());

    assert_eq!(paths, vec![PathBuf::from("/project/outline/blade.yml")]);
  }
}
