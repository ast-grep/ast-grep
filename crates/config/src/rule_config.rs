use crate::referent_rule::{ReferentRule, ReferentRuleError, RuleRegistration};
use crate::relational_rule::{Follows, Has, Inside, Precedes};
use crate::serialized_rule::{
  AtomicRule, CompositeRule, PatternStyle, RelationalRule, SerializableRule,
};

pub use crate::constraints::{
  try_deserialize_matchers, try_from_serializable as deserialize_meta_var, RuleWithConstraint,
  SerializableMetaVarMatcher, SerializeConstraintsError,
};
use ast_grep_core::language::Language;
use ast_grep_core::matcher::{KindMatcher, KindMatcherError, RegexMatcher, RegexMatcherError};
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::ops as o;
use ast_grep_core::replace_meta_var_in_string;
use ast_grep_core::NodeMatch;
use ast_grep_core::{Matcher, Node, Pattern, PatternError};
use bit_set::BitSet;
use serde::{Deserialize, Serialize};
use serde_yaml::{with::singleton_map_recursive::deserialize, Deserializer, Error as YamlError};
use thiserror::Error;

use std::collections::HashMap;
use std::ops::Deref;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
  Hint,
  Info,
  Warning,
  Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableRuleConfig<L: Language> {
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Main message highlighting why this rule fired. It should be single line and concise,
  /// but specific enough to be understood without additional context.
  pub message: String,
  /// Additional notes to elaborate the message and provide potential fix to the issue.
  pub note: Option<String>,
  /// One of: Info, Warning, or Error
  pub severity: Severity,
  /// Specify the language to parse and the file extension to includ in matching.
  pub language: L,
  /// Pattern rules to find matching AST nodes
  pub rule: SerializableRule,
  /// A pattern to auto fix the issue. It can reference metavariables appeared in rule.
  pub fix: Option<String>,
  /// Addtional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
  /// Utility rules that can be used in `matches`
  pub utils: Option<HashMap<String, SerializableRule>>,
  /// Glob patterns to specify that the rule only applies to matching files
  pub files: Option<Vec<String>>,
  /// Glob patterns that exclude rules from applying to files
  pub ignores: Option<Vec<String>>,
  /// Documentation link to this rule
  pub url: Option<String>,
  /// Extra information for the rule
  pub metadata: Option<HashMap<String, String>>,
}

type RResult<T> = Result<T, RuleConfigError>;

impl<L: Language> SerializableRuleConfig<L> {
  pub fn get_matcher(&self) -> RResult<RuleWithConstraint<L>> {
    let rule = self.get_rule()?;
    let matchers = self.get_meta_var_matchers()?;
    Ok(RuleWithConstraint::new(rule, matchers))
  }

  fn get_util_rules(&self) -> RResult<RuleRegistration<L>> {
    let registration = RuleRegistration::default();
    let env = DeserializeEnv::new(self.language.clone()).register_utils(registration.clone());
    if let Some(utils) = &self.utils {
      for (id, rule) in utils {
        let rule = RuleWithConstraint::new(
          deserialize_rule(rule.clone(), &env)?,
          MetaVarMatchers::default(),
        );
        registration.insert_rule(id, rule).unwrap();
      }
    }
    Ok(registration)
  }

  fn get_rule(&self) -> RResult<Rule<L>> {
    // TODO: add rules
    let registration = self.get_util_rules()?;
    let env = DeserializeEnv::new(self.language.clone()).register_utils(registration);
    Ok(deserialize_rule(self.rule.clone(), &env)?)
  }

  pub fn get_fixer(&self) -> RResult<Option<Pattern<L>>> {
    if let Some(fix) = &self.fix {
      Ok(Some(Pattern::try_new(fix, self.language.clone())?))
    } else {
      Ok(None)
    }
  }

  fn get_meta_var_matchers(&self) -> RResult<MetaVarMatchers<L>> {
    Ok(if let Some(constraints) = self.constraints.clone() {
      try_deserialize_matchers(constraints, self.language.clone())?
    } else {
      MetaVarMatchers::default()
    })
  }

  fn get_message(&self, node: &NodeMatch<L>) -> String {
    replace_meta_var_in_string(&self.message, node.get_env(), node.lang())
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

impl<L: Language> TryFrom<SerializableRuleConfig<L>> for RuleConfig<L> {
  type Error = RuleConfigError;
  fn try_from(inner: SerializableRuleConfig<L>) -> Result<Self, Self::Error> {
    let matcher = inner.get_matcher()?;
    let fixer = inner.get_fixer()?;
    Ok(Self {
      inner,
      matcher,
      fixer,
    })
  }
}

impl<L: Language> RuleConfig<L> {
  pub fn deserialize_str<'de>(yaml_str: &'de str) -> Result<Self, RuleConfigError>
  where
    L: Deserialize<'de>,
  {
    let deserializer = Deserializer::from_str(yaml_str);
    Self::deserialize(deserializer)
  }

  pub fn deserialize<'de>(deserializer: Deserializer<'de>) -> Result<Self, RuleConfigError>
  where
    L: Deserialize<'de>,
  {
    let inner: SerializableRuleConfig<L> = deserialize(deserializer)?;
    Self::try_from(inner)
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

pub enum Rule<L: Language> {
  // atomic
  Pattern(Pattern<L>),
  Kind(KindMatcher<L>),
  Regex(RegexMatcher<L>),
  // relational
  Inside(Box<Inside<L>>),
  Has(Box<Has<L>>),
  Precedes(Box<Precedes<L>>),
  Follows(Box<Follows<L>>),
  // composite
  All(o::All<L, Rule<L>>),
  Any(o::Any<L, Rule<L>>),
  Not(Box<o::Not<L, Rule<L>>>),
  Matches(ReferentRule<L>),
}
impl<L: Language> Rule<L> {
  pub fn is_atomic(&self) -> bool {
    use Rule::*;
    matches!(self, Pattern(_) | Kind(_) | Regex(_))
  }
  pub fn is_relational(&self) -> bool {
    use Rule::*;
    matches!(self, Inside(_) | Has(_) | Precedes(_) | Follows(_))
  }

  pub fn is_composite(&self) -> bool {
    use Rule::*;
    matches!(self, All(_) | Any(_) | Not(_) | Matches(_))
  }

  pub(crate) fn check_cyclic(&self, id: &str) -> bool {
    match self {
      Rule::All(all) => all.inner().iter().any(|r| r.check_cyclic(id)),
      Rule::Any(any) => any.inner().iter().any(|r| r.check_cyclic(id)),
      Rule::Not(not) => not.inner().check_cyclic(id),
      Rule::Matches(m) => m.rule_id == id,
      rule => {
        debug_assert!(!rule.is_composite());
        false
      }
    }
  }
}

impl<L: Language> Matcher<L> for Rule<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    use Rule::*;
    match self {
      // atomic
      Pattern(pattern) => pattern.match_node_with_env(node, env),
      Kind(kind) => kind.match_node_with_env(node, env),
      Regex(regex) => regex.match_node_with_env(node, env),
      // relational
      Inside(parent) => match_and_add_label(&**parent, node, env),
      Has(child) => match_and_add_label(&**child, node, env),
      Precedes(latter) => match_and_add_label(&**latter, node, env),
      Follows(former) => match_and_add_label(&**former, node, env),
      // composite
      All(all) => all.match_node_with_env(node, env),
      Any(any) => any.match_node_with_env(node, env),
      Not(not) => not.match_node_with_env(node, env),
      Matches(rule) => rule.match_node_with_env(node, env),
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    use Rule::*;
    match self {
      // atomic
      Pattern(pattern) => pattern.potential_kinds(),
      Kind(kind) => kind.potential_kinds(),
      Regex(regex) => regex.potential_kinds(),
      // relational
      Inside(parent) => parent.potential_kinds(),
      Has(child) => child.potential_kinds(),
      Precedes(latter) => latter.potential_kinds(),
      Follows(former) => former.potential_kinds(),
      // composite
      All(all) => all.potential_kinds(),
      Any(any) => any.potential_kinds(),
      Not(not) => not.potential_kinds(),
      Matches(rule) => rule.potential_kinds(),
    }
  }
}

/// Rule matches nothing by default.
/// In Math jargon, Rule is vacuously false.
impl<L: Language> Default for Rule<L> {
  fn default() -> Self {
    Self::Any(o::Any::new(std::iter::empty()))
  }
}

fn match_and_add_label<'tree, L: Language, M: Matcher<L>>(
  inner: &M,
  node: Node<'tree, L>,
  env: &mut MetaVarEnv<'tree, L>,
) -> Option<Node<'tree, L>> {
  let matched = inner.match_node_with_env(node, env)?;
  env.add_label("secondary", matched.clone());
  Some(matched)
}

#[derive(Debug, Error)]
pub enum RuleSerializeError {
  #[error("Rule must have one positive matcher.")]
  MissPositiveMatcher,
  #[error("Rule contains invalid kind matcher.")]
  InvalidKind(#[from] KindMatcherError),
  #[error("Rule contains invalid pattern matcher.")]
  InvalidPattern(#[from] PatternError),
  #[error("Rule contains invalid regex matcher.")]
  WrongRegex(#[from] RegexMatcherError),
  #[error("Rule contains invalid matches reference.")]
  MatchesRefrence(#[from] ReferentRuleError),
  #[error("field is only supported in has/inside.")]
  FieldNotSupported,
}

pub struct DeserializeEnv<L: Language> {
  registration: RuleRegistration<L>,
  lang: L,
}

impl<L: Language> DeserializeEnv<L> {
  pub fn new(lang: L) -> Self {
    Self {
      registration: Default::default(),
      lang,
    }
  }

  pub fn register_utils(mut self, registration: RuleRegistration<L>) -> Self {
    self.registration = registration;
    self
  }
}

// TODO: implement positive/non positive
pub fn deserialize_rule<L: Language>(
  serialized: SerializableRule,
  env: &DeserializeEnv<L>,
) -> Result<Rule<L>, RuleSerializeError> {
  let mut rules = Vec::with_capacity(1);
  use Rule as R;
  let categorized = serialized.categorized();
  deserialze_atomic_rule(categorized.atomic, &mut rules, env)?;
  deserialize_relational_rule(categorized.relational, &mut rules, env)?;
  deserialze_composite_rule(categorized.composite, &mut rules, env)?;
  if rules.is_empty() {
    Err(RuleSerializeError::MissPositiveMatcher)
  } else if rules.len() == 1 {
    Ok(rules.pop().expect("should not be empty"))
  } else {
    Ok(R::All(o::All::new(rules)))
  }
}

fn deserialze_composite_rule<L: Language>(
  composite: CompositeRule,
  rules: &mut Vec<Rule<L>>,
  env: &DeserializeEnv<L>,
) -> Result<(), RuleSerializeError> {
  use Rule as R;
  let convert_rules = |rules: Vec<SerializableRule>| -> Result<_, RuleSerializeError> {
    let mut inner = Vec::with_capacity(rules.len());
    for rule in rules {
      inner.push(deserialize_rule(rule, env)?);
    }
    Ok(inner)
  };
  if let Some(all) = composite.all {
    rules.push(R::All(o::All::new(convert_rules(all)?)));
  }
  if let Some(any) = composite.any {
    rules.push(R::Any(o::Any::new(convert_rules(any)?)));
  }
  if let Some(not) = composite.not {
    let not = o::Not::new(deserialize_rule(*not, env)?);
    rules.push(R::Not(Box::new(not)));
  }
  if let Some(id) = composite.matches {
    let matches = ReferentRule::try_new(id, &env.registration)?;
    rules.push(R::Matches(matches));
  }
  Ok(())
}

fn deserialize_relational_rule<L: Language>(
  relational: RelationalRule,
  rules: &mut Vec<Rule<L>>,
  env: &DeserializeEnv<L>,
) -> Result<(), RuleSerializeError> {
  use Rule as R;
  // relational
  if let Some(inside) = relational.inside {
    rules.push(R::Inside(Box::new(Inside::try_new(*inside, env)?)));
  }
  if let Some(has) = relational.has {
    rules.push(R::Has(Box::new(Has::try_new(*has, env)?)));
  }
  if let Some(precedes) = relational.precedes {
    rules.push(R::Precedes(Box::new(Precedes::try_new(*precedes, env)?)));
  }
  if let Some(follows) = relational.follows {
    rules.push(R::Follows(Box::new(Follows::try_new(*follows, env)?)));
  }
  Ok(())
}

fn deserialze_atomic_rule<L: Language>(
  atomic: AtomicRule,
  rules: &mut Vec<Rule<L>>,
  env: &DeserializeEnv<L>,
) -> Result<(), RuleSerializeError> {
  use Rule as R;
  if let Some(pattern) = atomic.pattern {
    rules.push(match pattern {
      PatternStyle::Str(pat) => R::Pattern(Pattern::try_new(&pat, env.lang.clone())?),
      PatternStyle::Contextual { context, selector } => {
        R::Pattern(Pattern::contextual(&context, &selector, env.lang.clone())?)
      }
    });
  }
  if let Some(kind) = atomic.kind {
    rules.push(R::Kind(KindMatcher::try_new(&kind, env.lang.clone())?));
  }
  if let Some(regex) = atomic.regex {
    rules.push(R::Regex(RegexMatcher::try_new(&regex)?));
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;

  fn ts_rule_config(rule: SerializableRule) -> SerializableRuleConfig<TypeScript> {
    SerializableRuleConfig {
      id: "".into(),
      message: "".into(),
      note: None,
      severity: Severity::Hint,
      language: TypeScript::Tsx,
      rule,
      fix: None,
      constraints: None,
      utils: None,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }

  #[test]
  fn test_rule_message() {
    let rule = from_str("pattern: class $A {}").expect("cannot parse rule");
    let config = SerializableRuleConfig {
      id: "test".into(),
      message: "Found $A".into(),
      ..ts_rule_config(rule)
    };
    let grep = TypeScript::Tsx.ast_grep("class TestClass {}");
    let node_match = grep
      .root()
      .find(config.get_matcher().unwrap())
      .expect("should find match");
    assert_eq!(config.get_message(&node_match), "Found TestClass");
  }

  #[test]
  fn test_augmented_rule() {
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  pattern: function test() { $$$ }
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("console.log(1)");
    assert!(grep.root().find(config.get_matcher().unwrap()).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    assert!(grep.root().find(config.get_matcher().unwrap()).is_some());
  }

  #[test]
  fn test_multiple_augment_rule() {
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  pattern: function test() { $$$ }
has:
  pattern: '123'
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    assert!(grep.root().find(config.get_matcher().unwrap()).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(123) }");
    assert!(grep.root().find(config.get_matcher().unwrap()).is_some());
  }

  #[test]
  fn test_rule_env() {
    let rule = from_str(
      "
all:
  - pattern: console.log($A)
  - inside:
      pattern: function $B() {$$$}
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    let node_match = grep
      .root()
      .find(config.get_matcher().unwrap())
      .expect("should found");
    let env = node_match.get_env();
    let a = env.get_match("A").expect("should exist").text();
    assert_eq!(a, "1");
    let b = env.get_match("B").expect("should exist").text();
    assert_eq!(b, "test");
  }
}
