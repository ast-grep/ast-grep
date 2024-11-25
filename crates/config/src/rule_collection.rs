use crate::{RuleConfig, Severity};
use ast_grep_core::language::Language;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

/// RuleBucket stores rules of the same language id.
/// Rules for different language will stay in separate buckets.
pub struct RuleBucket<L: Language> {
  rules: Vec<RuleConfig<L>>,
  lang: L,
}

impl<L: Language> RuleBucket<L> {
  fn new(lang: L) -> Self {
    Self {
      rules: vec![],
      lang,
    }
  }
  pub fn add(&mut self, rule: RuleConfig<L>) {
    self.rules.push(rule);
  }
}

struct ContingentRule<L: Language> {
  rule: RuleConfig<L>,
  files_globs: Option<GlobSet>,
  ignore_globs: Option<GlobSet>,
}

fn build_glob_set(paths: &Vec<String>) -> Result<GlobSet, globset::Error> {
  let mut builder = GlobSetBuilder::new();
  for path in paths {
    builder.add(Glob::new(path)?);
  }
  builder.build()
}

impl<L> TryFrom<RuleConfig<L>> for ContingentRule<L>
where
  L: Language,
{
  type Error = globset::Error;
  fn try_from(rule: RuleConfig<L>) -> Result<Self, Self::Error> {
    let files_globs = rule.files.as_ref().map(build_glob_set).transpose()?;
    let ignore_globs = rule.ignores.as_ref().map(build_glob_set).transpose()?;
    Ok(Self {
      rule,
      files_globs,
      ignore_globs,
    })
  }
}

impl<L: Language> ContingentRule<L> {
  pub fn matches_path<P: AsRef<Path>>(&self, path: P) -> bool {
    if let Some(ignore_globs) = &self.ignore_globs {
      if ignore_globs.is_match(&path) {
        return false;
      }
    }
    if let Some(files_globs) = &self.files_globs {
      return files_globs.is_match(path);
    }
    true
  }
}

/// A collection of rules to run one round of scanning.
/// Rules will be grouped together based on their language, path globbing and pattern rule.
pub struct RuleCollection<L: Language + Eq> {
  // use vec since we don't have many languages
  /// a list of rule buckets grouped by languages.
  /// Tenured rules will always run against a file of that language type.
  tenured: Vec<RuleBucket<L>>,
  /// contingent rules will run against a file if it matches file/ignore glob.
  contingent: Vec<ContingentRule<L>>,
}

impl<L: Language + Eq> RuleCollection<L> {
  pub fn try_new(configs: Vec<RuleConfig<L>>) -> Result<Self, globset::Error> {
    let mut tenured = vec![];
    let mut contingent = vec![];
    for config in configs {
      if matches!(config.severity, Severity::Off) {
        continue;
      } else if config.files.is_none() && config.ignores.is_none() {
        Self::add_tenured_rule(&mut tenured, config);
      } else {
        contingent.push(ContingentRule::try_from(config)?);
      }
    }
    Ok(Self {
      tenured,
      contingent,
    })
  }

  pub fn get_rule_from_lang(&self, path: &Path, lang: L) -> Vec<&RuleConfig<L>> {
    let mut all_rules = vec![];
    for rule in &self.tenured {
      if rule.lang == lang {
        all_rules = rule.rules.iter().collect();
        break;
      }
    }
    all_rules.extend(self.contingent.iter().filter_map(|cont| {
      if cont.rule.language == lang && cont.matches_path(path) {
        Some(&cont.rule)
      } else {
        None
      }
    }));
    all_rules
  }

  pub fn for_path<P: AsRef<Path>>(&self, path: P) -> Vec<&RuleConfig<L>> {
    let path = path.as_ref();
    let Some(lang) = L::from_path(path) else {
      return vec![];
    };
    let mut ret = self.get_rule_from_lang(path, lang);
    ret.sort_unstable_by_key(|r| &r.id);
    ret
  }

  pub fn get_rule(&self, id: &str) -> Option<&RuleConfig<L>> {
    for rule in &self.tenured {
      for r in &rule.rules {
        if r.id == id {
          return Some(r);
        }
      }
    }
    for rule in &self.contingent {
      if rule.rule.id == id {
        return Some(&rule.rule);
      }
    }
    None
  }

  pub fn total_rule_count(&self) -> usize {
    let mut ret = self.tenured.iter().map(|bucket| bucket.rules.len()).sum();
    ret += self.contingent.len();
    ret
  }

  pub fn for_each_rule(&self, mut f: impl FnMut(&RuleConfig<L>)) {
    for bucket in &self.tenured {
      for rule in &bucket.rules {
        f(rule);
      }
    }
    for rule in &self.contingent {
      f(&rule.rule);
    }
  }

  fn add_tenured_rule(tenured: &mut Vec<RuleBucket<L>>, rule: RuleConfig<L>) {
    let lang = rule.language.clone();
    for bucket in tenured.iter_mut() {
      if bucket.lang == lang {
        bucket.add(rule);
        return;
      }
    }
    let mut bucket = RuleBucket::new(lang);
    bucket.add(rule);
    tenured.push(bucket);
  }
}

impl<L: Language + Eq> Default for RuleCollection<L> {
  fn default() -> Self {
    Self {
      tenured: vec![],
      contingent: vec![],
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_yaml_string;
  use crate::test::TypeScript;
  use crate::GlobalRules;

  fn make_rule(files: &str) -> RuleCollection<TypeScript> {
    let globals = GlobalRules::default();
    let rule_config = from_yaml_string(
      &format!(
        r"
id: test
message: test rule
severity: info
language: Tsx
rule:
  all: [kind: number]
{files}"
      ),
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap();
    RuleCollection::try_new(vec![rule_config]).expect("should parse")
  }

  fn assert_match_path(collection: &RuleCollection<TypeScript>, path: &str) {
    let rules = collection.for_path(path);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].id, "test");
  }

  fn assert_ignore_path(collection: &RuleCollection<TypeScript>, path: &str) {
    let rules = collection.for_path(path);
    assert!(rules.is_empty());
  }

  #[test]
  fn test_ignore_rule() {
    let src = r#"
ignores:
  - ./manage.py
  - "**/test*"
"#;
    let collection = make_rule(src);
    assert_ignore_path(&collection, "./manage.py");
    assert_ignore_path(&collection, "./src/test.py");
    assert_match_path(&collection, "./src/app.py");
  }

  #[test]
  fn test_files_rule() {
    let src = r#"
files:
  - ./manage.py
  - "**/test*"
"#;
    let collection = make_rule(src);
    assert_match_path(&collection, "./manage.py");
    assert_match_path(&collection, "./src/test.py");
    assert_ignore_path(&collection, "./src/app.py");
  }

  #[test]
  fn test_files_with_ignores_rule() {
    let src = r#"
files:
  - ./src/**/*.py
ignores:
  - ./src/excluded/*.py
"#;
    let collection = make_rule(src);
    assert_match_path(&collection, "./src/test.py");
    assert_match_path(&collection, "./src/some_folder/test.py");
    assert_ignore_path(&collection, "./src/excluded/app.py");
  }

  #[test]
  fn test_rule_collection_get_contingent_rule() {
    let src = r#"
files:
  - ./manage.py
  - "**/test*"
"#;
    let collection = make_rule(src);
    assert!(collection.get_rule("test").is_some());
  }

  #[test]
  fn test_rule_collection_get_tenured_rule() {
    let src = r#""#;
    let collection = make_rule(src);
    assert!(collection.get_rule("test").is_some());
  }

  #[test]
  #[ignore]
  fn test_rules_for_path() {
    todo!()
  }
}
