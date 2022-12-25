use crate::error::ErrorContext as EC;
use crate::verify::{SnapshotCollection, TestCase, TestSnapshots};
use anyhow::{Context, Result};
use ast_grep_config::{from_str, from_yaml_string, RuleCollection, RuleConfig};
use ast_grep_language::{config_file_type, SupportLang};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestConfig {
  test_dir: PathBuf,
  /// Specify the directory containing snapshots. The path is relative to `test_dir`
  snapshot_dir: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AstGrepConfig {
  /// YAML rule directories
  pub rule_dirs: Vec<PathBuf>,
  /// test configurations
  pub test_configs: Option<Vec<TestConfig>>,
  /// overriding config for rules
  pub rules: Option<Vec<()>>,
}

pub fn find_config(config_path: Option<PathBuf>) -> Result<RuleCollection<SupportLang>> {
  let config_path = find_config_path_with_default(config_path).context(EC::ReadConfiguration)?;
  let config_str = read_to_string(&config_path).context(EC::ReadConfiguration)?;
  let sg_config: AstGrepConfig = from_str(&config_str).context(EC::ParseConfiguration)?;
  let base_dir = config_path
    .parent()
    .expect("config file must have parent directory");
  read_directory_yaml(base_dir, sg_config)
}

fn read_directory_yaml(
  base_dir: &Path,
  sg_config: AstGrepConfig,
) -> Result<RuleCollection<SupportLang>> {
  let mut configs = vec![];
  for dir in sg_config.rule_dirs {
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
      let new_configs = read_rule_file(path)?;
      configs.extend(new_configs);
    }
  }
  RuleCollection::try_new(configs).context(EC::GlobPattern)
}

pub fn read_rule_file(path: &Path) -> Result<Vec<RuleConfig<SupportLang>>> {
  let yaml = read_to_string(path).with_context(|| EC::ReadRule(path.to_path_buf()))?;
  from_yaml_string(&yaml).with_context(|| EC::ParseRule(path.to_path_buf()))
}

pub struct TestHarness {
  pub test_cases: Vec<TestCase>,
  pub snapshots: SnapshotCollection,
  pub path_map: HashMap<String, PathBuf>,
}

pub fn find_tests(config_path: Option<PathBuf>) -> Result<TestHarness> {
  let config_path = find_config_path_with_default(config_path).context(EC::ReadConfiguration)?;
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

const CONFIG_FILE: &str = "sgconfig.yml";
const SNAPSHOT_DIR: &str = "__snapshots__";

fn find_config_path_with_default(config_path: Option<PathBuf>) -> Result<PathBuf> {
  if let Some(config) = config_path {
    return Ok(config);
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
