use crate::deserialize_env::DeserializeEnv;
use crate::referent_rule::GlobalRules;
use crate::rule::{deserialize_rule, RuleSerializeError, SerializableRule};

pub use crate::constraints::{
  try_deserialize_matchers, try_from_serializable as deserialize_meta_var, RuleWithConstraint,
  SerializableMetaVarMatcher, SerializeConstraintsError,
};
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::replace_meta_var_in_string;
use ast_grep_core::NodeMatch;
use ast_grep_core::{Pattern, PatternError};
use serde::{Deserialize, Serialize};
use serde_yaml::{with::singleton_map_recursive::deserialize, Deserializer, Error as YamlError};
use thiserror::Error;

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
  Hint,
  Info,
  Warning,
  Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableRuleCore<L: Language> {
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Specify the language to parse and the file extension to includ in matching.
  pub language: L,
  /// Pattern rules to find matching AST nodes
  pub rule: SerializableRule,
  /// Addtional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
  /// Utility rules that can be used in `matches`
  pub utils: Option<HashMap<String, SerializableRule>>,
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

  fn get_meta_var_matchers(&self) -> RResult<MetaVarMatchers<L>> {
    Ok(if let Some(constraints) = self.constraints.clone() {
      try_deserialize_matchers(constraints, self.language.clone())?
    } else {
      MetaVarMatchers::default()
    })
  }

  pub fn get_matcher(&self, globals: &GlobalRules<L>) -> RResult<RuleWithConstraint<L>> {
    let env = self.get_deserialize_env(globals)?;
    let rule = deserialize_rule(self.rule.clone(), &env)?;
    let matchers = self.get_meta_var_matchers()?;
    Ok(
      RuleWithConstraint::new(rule)
        .with_matchers(matchers)
        .with_utils(env.registration),
    )
  }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableRuleConfig<L: Language> {
  #[serde(flatten)]
  pub core: SerializableRuleCore<L>,
  /// Main message highlighting why this rule fired. It should be single line and concise,
  /// but specific enough to be understood without additional context.
  pub message: String,
  /// Additional notes to elaborate the message and provide potential fix to the issue.
  pub note: Option<String>,
  /// One of: Info, Warning, or Error
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
  fn get_fixer(&self) -> RResult<Option<Pattern<L>>> {
    if let Some(fix) = &self.fix {
      Ok(Some(Pattern::try_new(fix, self.language.clone())?))
    } else {
      Ok(None)
    }
  }

  fn get_message(&self, node: &NodeMatch<L>) -> String {
    replace_meta_var_in_string(&self.message, node.get_env(), node.lang())
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
  Fixer(#[from] PatternError),
  #[error("constraints is not configured correctly.")]
  Constraints(#[from] SerializeConstraintsError),
}

pub struct RuleConfig<L: Language> {
  inner: SerializableRuleConfig<L>,
  pub matcher: RuleWithConstraint<L>,
  pub fixer: Option<Pattern<L>>,
}

impl<L: Language> RuleConfig<L> {
  // pub fn deserialize_str<'de>(yaml_str: &'de str) -> Result<Self, RuleConfigError>
  // where
  //   L: Deserialize<'de>,
  // {
  //   let deserializer = Deserializer::from_str(yaml_str);
  //   Self::deserialize(deserializer)
  // }

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

  pub fn get_message(&self, node: &NodeMatch<L>) -> String {
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
      id: "".into(),
      language: TypeScript::Tsx,
      rule,
      constraints: None,
      utils: None,
    };
    SerializableRuleConfig {
      core,
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
