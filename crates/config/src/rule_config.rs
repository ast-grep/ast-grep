use crate::deserialize_env::DeserializeEnv;
use crate::referent_rule::GlobalRules;
use crate::rule::{RuleSerializeError, SerializableRule};
use crate::transform::Transformation;

pub use crate::constraints::{
  try_deserialize_matchers, try_from_serializable as deserialize_meta_var, RuleWithConstraint,
  SerializableMetaVarMatcher, SerializeConstraintsError,
};
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::replacer::Replacer;
use ast_grep_core::replacer::{Fixer, FixerError};
use ast_grep_core::{NodeMatch, StrDoc};
use serde::{Deserialize, Serialize};
use serde_yaml::{with::singleton_map_recursive::deserialize, Deserializer, Error as YamlError};
use thiserror::Error;

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

// type Pattern<L> = PatternCore<StrDoc<L>>;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
  #[default]
  /// A kind reminder for code with potential improvement.
  Hint,
  /// A suggestion that code can be improved or optimized.
  Info,
  /// A warning that code might produce bugs or does not follow best practice.
  Warning,
  /// An error that code produces bugs or has logic errors.
  Error,
  /// Turns off the rule.
  Off,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableRuleCore<L: Language> {
  /// Specify the language to parse and the file extension to includ in matching.
  pub language: L,
  /// Pattern rules to find matching AST nodes
  pub rule: SerializableRule,
  /// Addtional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
  /// Utility rules that can be used in `matches`
  pub utils: Option<HashMap<String, SerializableRule>>,
  /// A dictionary for meatvariable manipulation. Dict key is the new variable name.
  /// Dict value is a [transformation] that specifies how meta var is processed.
  /// Warning: this is experimental option. [`https://github.com/ast-grep/ast-grep/issues/436`]
  pub transform: Option<HashMap<String, Transformation>>,
}

impl<L: Language> SerializableRuleCore<L> {
  fn get_deserialize_env(&self, globals: &GlobalRules<L>) -> RResult<DeserializeEnv<L>> {
    let env = DeserializeEnv::new(self.language.clone()).with_globals(globals);
    if let Some(utils) = &self.utils {
      let env = env.register_local_utils(utils)?;
      Ok(env)
    } else {
      Ok(env)
    }
  }

  fn get_meta_var_matchers(&self) -> RResult<MetaVarMatchers<StrDoc<L>>> {
    Ok(if let Some(constraints) = self.constraints.clone() {
      try_deserialize_matchers(constraints, self.language.clone())?
    } else {
      MetaVarMatchers::default()
    })
  }

  pub fn get_matcher(&self, globals: &GlobalRules<L>) -> RResult<RuleWithConstraint<L>> {
    let env = self.get_deserialize_env(globals)?;
    let rule = env.deserialize_rule(self.rule.clone())?;
    let matchers = self.get_meta_var_matchers()?;
    let transform = self.transform.clone();
    Ok(
      RuleWithConstraint::new(rule)
        .with_matchers(matchers)
        .with_utils(env.registration)
        .with_transform(transform),
    )
  }
}
#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableRuleConfigCore<L: Language> {
  #[serde(flatten)]
  pub core: SerializableRuleCore<L>,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
}

pub fn into_map<L: Language>(
  rules: Vec<SerializableRuleConfigCore<L>>,
) -> HashMap<String, SerializableRuleCore<L>> {
  rules.into_iter().map(|r| (r.id, r.core)).collect()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableRuleConfig<L: Language> {
  #[serde(flatten)]
  pub core: SerializableRuleCore<L>,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Main message highlighting why this rule fired. It should be single line and concise,
  /// but specific enough to be understood without additional context.
  #[serde(default)]
  pub message: String,
  /// Additional notes to elaborate the message and provide potential fix to the issue.
  pub note: Option<String>,
  /// One of: hint, info, warning, or error
  #[serde(default)]
  pub severity: Severity,
  /// A pattern to auto fix the issue. It can reference metavariables appeared in rule.
  pub fix: Option<String>,
  /// Glob patterns to specify that the rule only applies to matching files
  pub files: Option<Vec<String>>,
  /// Glob patterns that exclude rules from applying to files
  pub ignores: Option<Vec<String>>,
  /// Documentation link to this rule
  pub url: Option<String>,
  /// Extra information for the rule
  pub metadata: Option<HashMap<String, String>>,
}

type RResult<T> = std::result::Result<T, RuleConfigError>;

impl<L: Language> SerializableRuleConfig<L> {
  fn get_fixer(&self) -> RResult<Option<Fixer<String>>> {
    if let Some(fix) = &self.fix {
      if let Some(trans) = &self.transform {
        let keys: Vec<_> = trans.keys().cloned().collect();
        Ok(Some(Fixer::with_transform(fix, &self.language, &keys)))
      } else {
        Ok(Some(Fixer::try_new(fix, &self.language)?))
      }
    } else {
      Ok(None)
    }
  }

  fn get_message(&self, node_match: &NodeMatch<StrDoc<L>>) -> String {
    let bytes = self.message.generate_replacement(node_match);
    String::from_utf8(bytes).expect("replacement must be valid utf-8")
  }
}

impl<L: Language> Deref for SerializableRuleConfig<L> {
  type Target = SerializableRuleCore<L>;
  fn deref(&self) -> &Self::Target {
    &self.core
  }
}

impl<L: Language> DerefMut for SerializableRuleConfig<L> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.core
  }
}

#[derive(Debug, Error)]
pub enum RuleConfigError {
  #[error("Fail to parse yaml as RuleConfig")]
  Yaml(#[from] YamlError),
  #[error("Rule is not configured correctly.")]
  Rule(#[from] RuleSerializeError),
  #[error("fix pattern is invalid.")]
  Fixer(#[from] FixerError),
  #[error("constraints is not configured correctly.")]
  Constraints(#[from] SerializeConstraintsError),
}

pub struct RuleConfig<L: Language> {
  inner: SerializableRuleConfig<L>,
  pub matcher: RuleWithConstraint<L>,
  pub fixer: Option<Fixer<String>>,
}

impl<L: Language> RuleConfig<L> {
  pub fn try_from(
    inner: SerializableRuleConfig<L>,
    globals: &GlobalRules<L>,
  ) -> Result<Self, RuleConfigError> {
    let matcher = inner.get_matcher(globals)?;
    let fixer = inner.get_fixer()?;
    Ok(Self {
      inner,
      matcher,
      fixer,
    })
  }

  pub fn deserialize<'de>(
    deserializer: Deserializer<'de>,
    globals: &GlobalRules<L>,
  ) -> Result<Self, RuleConfigError>
  where
    L: Deserialize<'de>,
  {
    let inner: SerializableRuleConfig<L> = deserialize(deserializer)?;
    Self::try_from(inner, globals)
  }

  pub fn get_message(&self, node: &NodeMatch<StrDoc<L>>) -> String {
    self.inner.get_message(node)
  }
}
impl<L: Language> Deref for RuleConfig<L> {
  type Target = SerializableRuleConfig<L>;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;

  fn ts_rule_config(rule: SerializableRule) -> SerializableRuleConfig<TypeScript> {
    let core = SerializableRuleCore {
      language: TypeScript::Tsx,
      rule,
      constraints: None,
      transform: None,
      utils: None,
    };
    SerializableRuleConfig {
      core,
      id: "".into(),
      message: "".into(),
      note: None,
      severity: Severity::Hint,
      fix: None,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }

  #[test]
  fn test_rule_message() {
    let globals = GlobalRules::default();
    let rule = from_str("pattern: class $A {}").expect("cannot parse rule");
    let mut config = ts_rule_config(rule);
    config.id = "test".into();
    config.message = "Found $A".into();
    let grep = TypeScript::Tsx.ast_grep("class TestClass {}");
    let node_match = grep
      .root()
      .find(config.get_matcher(&globals).unwrap())
      .expect("should find match");
    assert_eq!(config.get_message(&node_match), "Found TestClass");
  }

  #[test]
  fn test_augmented_rule() {
    let globals = GlobalRules::default();
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  stopBy: end
  pattern: function test() { $$$ }
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("console.log(1)");
    let matcher = config.get_matcher(&globals).unwrap();
    assert!(grep.root().find(&matcher).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    assert!(grep.root().find(&matcher).is_some());
  }

  #[test]
  fn test_multiple_augment_rule() {
    let globals = GlobalRules::default();
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  stopBy: end
  pattern: function test() { $$$ }
has:
  stopBy: end
  pattern: '123'
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    let matcher = config.get_matcher(&globals).unwrap();
    assert!(grep.root().find(&matcher).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(123) }");
    assert!(grep.root().find(&matcher).is_some());
  }

  #[test]
  fn test_rule_env() {
    let globals = GlobalRules::default();
    let rule = from_str(
      "
all:
  - pattern: console.log($A)
  - inside:
      stopBy: end
      pattern: function $B() {$$$}
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    let node_match = grep
      .root()
      .find(config.get_matcher(&globals).unwrap())
      .expect("should found");
    let env = node_match.get_env();
    let a = env.get_match("A").expect("should exist").text();
    assert_eq!(a, "1");
    let b = env.get_match("B").expect("should exist").text();
    assert_eq!(b, "test");
  }

  #[test]
  fn test_transform() {
    let globals = GlobalRules::default();
    let rule = from_str("pattern: console.log($A)").expect("should parse");
    let mut config = ts_rule_config(rule);
    let transform = from_str(
      "
B:
  substring:
    source: $A
    startChar: 1
    endChar: -1
",
    )
    .expect("should parse");
    config.transform = Some(transform);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(123) }");
    let node_match = grep
      .root()
      .find(config.get_matcher(&globals).unwrap())
      .expect("should found");
    let env = node_match.get_env();
    let a = env.get_match("A").expect("should exist").text();
    assert_eq!(a, "123");
    let b = env.get_transformed("B").expect("should exist");
    assert_eq!(b, b"2");
  }

  fn get_matches_config() -> SerializableRuleConfig<TypeScript> {
    let rule = from_str(
      "
matches: test-rule
",
    )
    .unwrap();
    let utils = from_str(
      "
test-rule:
  pattern: some($A)
",
    )
    .unwrap();
    let mut ret = ts_rule_config(rule);
    ret.utils = Some(utils);
    ret
  }

  #[test]
  fn test_utils_rule() {
    let globals = GlobalRules::default();
    let config = get_matches_config();
    let matcher = config.get_matcher(&globals).unwrap();
    let grep = TypeScript::Tsx.ast_grep("some(123)");
    assert!(grep.root().find(&matcher).is_some());
    let grep = TypeScript::Tsx.ast_grep("some()");
    assert!(grep.root().find(&matcher).is_none());
  }
}
