use crate::error::ErrorContext as EC;
use crate::lang::{CustomLang, SgLang};
use crate::verify::{SnapshotCollection, TestCase, TestSnapshots};
use anyhow::{Context, Result};
use ast_grep_config::{
  from_str, from_yaml_string, DeserializeEnv, GlobalRules, RuleCollection, RuleConfig,
};
use ast_grep_language::config_file_type;
use clap::ValueEnum;
use ignore::WalkBuilder;
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
  snapshot_dir: Option<PathBuf>,
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
  pub custom_languages: Option<HashMap<String, CustomLang>>, // /// overriding config for rules
                                                             // #[serde(skip_serializing_if="Option::is_none")]
                                                             // pub rules: Option<Vec<()>>,
}

pub fn find_rules(config_path: Option<PathBuf>) -> Result<RuleCollection<SgLang>> {
  let config_path =
    find_config_path_with_default(config_path, None).context(EC::ReadConfiguration)?;
  let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
  let sg_config: AstGrepConfig = from_str(&config_str).context(EC::ParseConfiguration)?;
  let base_dir = config_path
    .parent()
    .expect("config file must have parent directory");
  let global_rules = find_util_rules(base_dir, sg_config.util_dirs)?;
  read_directory_yaml(base_dir, sg_config.rule_dirs, global_rules)
}

// TODO: add error
pub fn register_custom_language(config_path: Option<PathBuf>) {
  let Ok(mut path) = find_config_path_with_default(config_path, None) else {
    return;
  };
  let Ok(config_str) = read_to_string(&path) else { return };
  let sg_config: AstGrepConfig = from_str(&config_str).unwrap();
  path.pop();
  if let Some(custom_langs) = sg_config.custom_languages {
    SgLang::register_custom_language(path, custom_langs);
  }
}

fn find_util_rules(
  base_dir: &Path,
  util_dirs: Option<Vec<PathBuf>>,
) -> Result<GlobalRules<SgLang>> {
  let Some(util_dirs) = util_dirs else {
    return Ok(GlobalRules::default())
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
  let ret = DeserializeEnv::parse_global_utils(utils).context("TODO!")?;
  Ok(ret)
}

fn read_directory_yaml(
  base_dir: &Path,
  rule_dirs: Vec<PathBuf>,
  global_rules: GlobalRules<SgLang>,
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
  RuleCollection::try_new(configs).context(EC::GlobPattern)
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

pub struct TestHarness {
  pub test_cases: Vec<TestCase>,
  pub snapshots: SnapshotCollection,
  pub path_map: HashMap<String, PathBuf>,
}

pub fn find_tests(config_path: Option<PathBuf>) -> Result<TestHarness> {
  let config_path =
    find_config_path_with_default(config_path, None).context(EC::ReadConfiguration)?;
  let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
  let sg_config: AstGrepConfig = from_str(&config_str).context(EC::ParseConfiguration)?;
  let base_dir = config_path
    .parent()
    .expect("config file must have parent directory");
  let test_configs = sg_config.test_configs.unwrap_or_default();
  let mut test_cases = vec![];
  let mut snapshots = SnapshotCollection::new();
  let mut path_map = HashMap::new();
  for test in test_configs {
    let TestHarness {
      test_cases: new_cases,
      snapshots: new_snapshots,
      path_map: new_path_map,
    } = read_test_files(base_dir, &test.test_dir, test.snapshot_dir.as_deref())?;
    path_map.extend(new_path_map);
    test_cases.extend(new_cases);
    snapshots.extend(new_snapshots);
  }
  Ok(TestHarness {
    test_cases,
    snapshots,
    path_map,
  })
}

pub fn read_test_files(
  base_dir: &Path,
  test_dir: &Path,
  snapshot_dir: Option<&Path>,
) -> Result<TestHarness> {
  let mut test_cases = vec![];
  let mut snapshots = HashMap::new();
  let mut path_map = HashMap::new();
  let dir_path = base_dir.join(test_dir);
  let snapshot_dir = snapshot_dir.unwrap_or_else(|| SNAPSHOT_DIR.as_ref());
  let snapshot_dir = dir_path.join(snapshot_dir);
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
    let yaml = read_to_string(path).with_context(|| EC::ReadRule(path.to_path_buf()))?;
    if path.starts_with(&snapshot_dir) {
      let snapshot: TestSnapshots =
        from_str(&yaml).with_context(|| EC::ParseTest(path.to_path_buf()))?;
      snapshots.insert(snapshot.id.clone(), snapshot);
    } else {
      let test_case: TestCase =
        from_str(&yaml).with_context(|| EC::ParseTest(path.to_path_buf()))?;
      path_map.insert(test_case.id.clone(), dir_path.join(SNAPSHOT_DIR));
      test_cases.push(test_case);
    }
  }
  Ok(TestHarness {
    test_cases,
    snapshots,
    path_map,
  })
}

pub fn read_config_from_dir<P: AsRef<Path>>(path: P) -> Result<Option<(PathBuf, AstGrepConfig)>> {
  let config_path =
    find_config_path_with_default(None, Some(path.as_ref())).context(EC::ReadConfiguration)?;
  if !config_path.is_file() {
    return Ok(None);
  }
  let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
  let sg_config = from_str(&config_str).context(EC::ParseConfiguration)?;
  Ok(Some((config_path, sg_config)))
}

const CONFIG_FILE: &str = "sgconfig.yml";
const SNAPSHOT_DIR: &str = "__snapshots__";

fn find_config_path_with_default(
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

#[derive(Clone, Copy, Deserialize, Serialize, ValueEnum)]
pub enum IgnoreFile {
  /// Search hidden files and directories. By default, hidden files and directories are skipped.
  Hidden,
  /// Don't respect .ignore files.
  /// This does *not* affect whether ripgrep will ignore files and directories whose names begin with a dot.
  /// For that, use --no-ignore hidden.
  Dot,
  /// Don't respect ignore files that are manually configured for the repository such as git's '.git/info/exclude'.
  Exclude,
  /// Don't respect ignore files that come from "global" sources such as git's
  /// `core.excludesFile` configuration option (which defaults to `$HOME/.config/git/ignore`).
  Global,
  /// Don't respect ignore files (.gitignore, .ignore, etc.) in parent directories.
  Parent,
  /// Don't respect version control ignore files (.gitignore, etc.).
  /// This implies --no-ignore parent for VCS files.
  /// Note that .ignore files will continue to be respected.
  Vcs,
}

#[derive(Default)]
pub struct NoIgnore {
  disregard_hidden: bool,
  disregard_parent: bool,
  disregard_dot: bool,
  disregard_vcs: bool,
  disregard_global: bool,
  disregard_exclude: bool,
}

impl NoIgnore {
  pub fn disregard(ignores: &Vec<IgnoreFile>) -> Self {
    let mut ret = NoIgnore::default();
    use IgnoreFile::*;
    for ignore in ignores {
      match ignore {
        Hidden => ret.disregard_hidden = true,
        Dot => ret.disregard_dot = true,
        Exclude => ret.disregard_exclude = true,
        Global => ret.disregard_global = true,
        Parent => ret.disregard_parent = true,
        Vcs => ret.disregard_vcs = true,
      }
    }
    ret
  }

  pub fn walk(&self, path: &[PathBuf]) -> WalkBuilder {
    let mut paths = path.iter();
    let mut builder = WalkBuilder::new(paths.next().expect("non empty"));
    for path in paths {
      builder.add(path);
    }
    builder
      .hidden(!self.disregard_hidden)
      .parents(!self.disregard_parent)
      .ignore(!self.disregard_dot)
      .git_global(!self.disregard_vcs && !self.disregard_global)
      .git_ignore(!self.disregard_vcs)
      .git_exclude(!self.disregard_vcs && !self.disregard_exclude);
    builder
  }
}
