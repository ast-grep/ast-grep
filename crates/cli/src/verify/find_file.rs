use super::{SnapshotCollection, TestCase, TestSnapshots};
use crate::config::{find_config_path_with_default, AstGrepConfig};
use crate::error::ErrorContext as EC;

use anyhow::{Context, Result};
use ast_grep_config::from_str;
use ast_grep_language::config_file_type;
use ignore::WalkBuilder;
use regex::Regex;
use serde_yaml::{with::singleton_map_recursive::deserialize, Deserializer};

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

const SNAPSHOT_DIR: &str = "__snapshots__";

#[derive(Default)]
pub struct TestHarness {
  pub test_cases: Vec<TestCase>,
  pub snapshots: SnapshotCollection,
  pub path_map: HashMap<String, PathBuf>,
}

struct HarnessBuilder<'a> {
  dest: TestHarness,
  // base_dir: PathBuf,
  regex_filter: Option<&'a Regex>,
}

impl<'a> HarnessBuilder<'a> {}

pub fn find_tests(
  config_path: Option<PathBuf>,
  regex_filter: Option<&Regex>,
) -> Result<TestHarness> {
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
    } = read_test_files(
      base_dir,
      &test.test_dir,
      test.snapshot_dir.as_deref(),
      regex_filter,
    )?;
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
  test_dirname: &Path,
  snapshot_dirname: Option<&Path>,
  regex_filter: Option<&Regex>,
) -> Result<TestHarness> {
  let mut builder = HarnessBuilder {
    dest: TestHarness::default(),
    regex_filter,
  };
  let test_path = base_dir.join(test_dirname);
  let snapshot_dirname = snapshot_dirname.unwrap_or_else(|| SNAPSHOT_DIR.as_ref());
  let snapshot_path = test_path.join(snapshot_dirname);
  let walker = WalkBuilder::new(&test_path)
    .types(config_file_type())
    .build();
  for dir in walker {
    let config_file = dir.with_context(|| EC::WalkRuleDir(test_path.clone()))?;
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
    if path.starts_with(&snapshot_path) {
      deserialize_snapshot_yaml(path, yaml, &mut builder)?;
    } else {
      deserialize_test_yaml(path, yaml, &snapshot_path, &mut builder)?;
    }
  }
  Ok(builder.dest)
}

fn deserialize_snapshot_yaml(
  path: &Path,
  yaml: String,
  builder: &mut HarnessBuilder<'_>,
) -> Result<()> {
  let snapshot: TestSnapshots =
    from_str(&yaml).with_context(|| EC::ParseTest(path.to_path_buf()))?;
  let included_in_filter = builder
    .regex_filter
    .map(|r| r.is_match(&snapshot.id))
    .unwrap_or(true);
  if !included_in_filter {
    return Ok(());
  }
  let id = snapshot.id.clone();
  let existing = builder.dest.snapshots.insert(id.clone(), snapshot);
  if existing.is_some() {
    eprintln!("Warning: found duplicate test case snapshot for `{id}`");
  }
  Ok(())
}

fn deserialize_test_yaml(
  path: &Path,
  yaml: String,
  snapshot_path: &Path,
  builder: &mut HarnessBuilder<'_>,
) -> Result<()> {
  for deser in Deserializer::from_str(&yaml) {
    let test_case: TestCase =
      deserialize(deser).with_context(|| EC::ParseTest(path.to_path_buf()))?;
    if builder
      .regex_filter
      .map(|r| r.is_match(&test_case.id))
      .unwrap_or(true)
    {
      let harness = &mut builder.dest;
      harness
        .path_map
        .insert(test_case.id.clone(), snapshot_path.to_path_buf());
      harness.test_cases.push(test_case);
    }
  }
  Ok(())
}
