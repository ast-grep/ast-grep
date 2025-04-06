use crate::check_var::{check_rule_with_hint, CheckHint};
use crate::fixer::{Fixer, FixerError, SerializableFixer};
use crate::rule::referent_rule::RuleRegistration;
use crate::rule::Rule;
use crate::rule::{RuleSerializeError, SerializableRule};
use crate::transform::{Transform, TransformError, Transformation};
use crate::DeserializeEnv;

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Matcher, Node};
use serde::{Deserialize, Serialize};
use serde_yaml::Error as YamlError;

use bit_set::BitSet;
use schemars::JsonSchema;
use thiserror::Error;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

#[derive(Debug, Error)]
pub enum RuleCoreError {
  #[error("Fail to parse yaml as RuleConfig")]
  Yaml(#[from] YamlError),
  #[error("`utils` is not configured correctly.")]
  Utils(#[source] RuleSerializeError),
  #[error("`rule` is not configured correctly.")]
  Rule(#[from] RuleSerializeError),
  #[error("`constraints` is not configured correctly.")]
  Constraints(#[source] RuleSerializeError),
  #[error("`transform` is not configured correctly.")]
  Transform(#[from] TransformError),
  #[error("`fix` pattern is invalid.")]
  Fixer(#[from] FixerError),
  #[error("Undefined meta var `{0}` used in `{1}`.")]
  UndefinedMetaVar(String, &'static str),
}

type RResult<T> = std::result::Result<T, RuleCoreError>;

/// Used for global rules, rewriters, and pyo3/napi
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRuleCore {
  /// A rule object to find matching AST nodes
  pub rule: SerializableRule,
  /// Additional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableRule>>,
  /// Utility rules that can be used in `matches`
  pub utils: Option<HashMap<String, SerializableRule>>,
  /// A dictionary for metavariable manipulation. Dict key is the new variable name.
  /// Dict value is a [transformation] that specifies how meta var is processed.
  /// See [transformation doc](https://ast-grep.github.io/reference/yaml/transformation.html).
  pub transform: Option<HashMap<String, Transformation>>,
  /// A pattern string or a FixConfig object to auto fix the issue.
  /// It can reference metavariables appeared in rule.
  /// See details in fix [object reference](https://ast-grep.github.io/reference/yaml/fix.html#fixconfig).
  pub fix: Option<SerializableFixer>,
}

impl SerializableRuleCore {
  /// This function assumes env's local is empty.
  fn get_deserialize_env<L: Language>(&self, env: DeserializeEnv<L>) -> RResult<DeserializeEnv<L>> {
    if let Some(utils) = &self.utils {
      let env = env.with_utils(utils).map_err(RuleCoreError::Utils)?;
      Ok(env)
    } else {
      Ok(env)
    }
  }

  fn get_constraints<L: Language>(
    &self,
    env: &DeserializeEnv<L>,
  ) -> RResult<HashMap<String, Rule<L>>> {
    let mut constraints = HashMap::new();
    let Some(serde_cons) = &self.constraints else {
      return Ok(constraints);
    };
    for (key, ser) in serde_cons {
      let constraint = env
        .deserialize_rule(ser.clone())
        .map_err(RuleCoreError::Constraints)?;
      constraints.insert(key.to_string(), constraint);
    }
    Ok(constraints)
  }

  fn get_fixer<L: Language>(&self, env: &DeserializeEnv<L>) -> RResult<Option<Fixer<L>>> {
    if let Some(fix) = &self.fix {
      let parsed = Fixer::parse(fix, env, &self.transform)?;
      Ok(Some(parsed))
    } else {
      Ok(None)
    }
  }

  fn get_matcher_from_env<L: Language>(&self, env: &DeserializeEnv<L>) -> RResult<RuleCore<L>> {
    let rule = env.deserialize_rule(self.rule.clone())?;
    let constraints = self.get_constraints(env)?;
    let transform = self
      .transform
      .as_ref()
      .map(|t| Transform::deserialize(t, env))
      .transpose()?;
    let fixer = self.get_fixer(env)?;
    Ok(
      RuleCore::new(rule)
        .with_matchers(constraints)
        .with_registration(env.registration.clone())
        .with_transform(transform)
        .with_fixer(fixer),
    )
  }

  pub fn get_matcher<L: Language>(&self, env: DeserializeEnv<L>) -> RResult<RuleCore<L>> {
    self.get_matcher_with_hint(env, CheckHint::Normal)
  }

  pub(crate) fn get_matcher_with_hint<L: Language>(
    &self,
    env: DeserializeEnv<L>,
    hint: CheckHint,
  ) -> RResult<RuleCore<L>> {
    let env = self.get_deserialize_env(env)?;
    let ret = self.get_matcher_from_env(&env)?;
    check_rule_with_hint(
      &ret.rule,
      &ret.registration,
      &ret.constraints,
      &self.transform,
      &ret.fixer,
      hint,
    )?;
    Ok(ret)
  }
}

pub struct RuleCore<L: Language> {
  rule: Rule<L>,
  constraints: HashMap<String, Rule<L>>,
  kinds: Option<BitSet>,
  pub(crate) transform: Option<Transform>,
  pub fixer: Option<Fixer<L>>,
  // this is required to hold util rule reference
  registration: RuleRegistration<L>,
}

impl<L: Language> RuleCore<L> {
  #[inline]
  pub fn new(rule: Rule<L>) -> Self {
    let kinds = rule.potential_kinds();
    Self {
      rule,
      kinds,
      ..Default::default()
    }
  }

  #[inline]
  pub fn with_matchers(self, constraints: HashMap<String, Rule<L>>) -> Self {
    Self {
      constraints,
      ..self
    }
  }

  #[inline]
  pub fn with_registration(self, registration: RuleRegistration<L>) -> Self {
    Self {
      registration,
      ..self
    }
  }

  #[inline]
  pub fn with_transform(self, transform: Option<Transform>) -> Self {
    Self { transform, ..self }
  }

  #[inline]
  pub fn with_fixer(self, fixer: Option<Fixer<L>>) -> Self {
    Self { fixer, ..self }
  }

  pub fn get_env(&self, lang: L) -> DeserializeEnv<L> {
    DeserializeEnv {
      lang,
      registration: self.registration.clone(),
    }
  }

  pub fn defined_vars(&self) -> HashSet<&str> {
    let mut ret = self.rule.defined_vars();
    for v in self.registration.get_local_util_vars() {
      ret.insert(v);
    }
    for constraint in self.constraints.values() {
      for var in constraint.defined_vars() {
        ret.insert(var);
      }
    }
    if let Some(trans) = &self.transform {
      for key in trans.keys() {
        ret.insert(key);
      }
    }
    ret
  }

  pub(crate) fn do_match<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
    enclosing_env: Option<&MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(kinds) = &self.kinds {
      if !kinds.contains(node.kind_id().into()) {
        return None;
      }
    }
    let ret = self.rule.match_node_with_env(node, env)?;
    if !env.to_mut().match_constraints(&self.constraints) {
      return None;
    }
    if let Some(trans) = &self.transform {
      let rewriters = self.registration.get_rewriters();
      let env = env.to_mut();
      if let Some(enclosing) = enclosing_env {
        trans.apply_transform(env, rewriters, enclosing);
      } else {
        let enclosing = env.clone();
        trans.apply_transform(env, rewriters, &enclosing);
      };
    }
    Some(ret)
  }
}
impl<L: Language> Deref for RuleCore<L> {
  type Target = Rule<L>;
  fn deref(&self) -> &Self::Target {
    &self.rule
  }
}

impl<L: Language> Default for RuleCore<L> {
  #[inline]
  fn default() -> Self {
    Self {
      rule: Rule::default(),
      constraints: HashMap::default(),
      kinds: None,
      transform: None,
      fixer: None,
      registration: RuleRegistration::default(),
    }
  }
}

impl<L: Language> Matcher<L> for RuleCore<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    self.do_match(node, env, None)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.rule.potential_kinds()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::rule::referent_rule::{ReferentRule, ReferentRuleError};
  use crate::test::TypeScript;
  use ast_grep_core::matcher::{Pattern, RegexMatcher};

  fn get_matcher(src: &str) -> RResult<RuleCore<TypeScript>> {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let rule: SerializableRuleCore = from_str(src).expect("should word");
    rule.get_matcher(env)
  }

  #[test]
  fn test_rule_error() {
    let ret = get_matcher(r"rule: {kind: bbb}");
    assert!(matches!(ret, Err(RuleCoreError::Rule(_))));
  }

  #[test]
  fn test_utils_error() {
    let ret = get_matcher(
      r"
rule: { kind: number }
utils: { testa: {kind: bbb} }
  ",
    );
    assert!(matches!(ret, Err(RuleCoreError::Utils(_))));
  }

  #[test]
  fn test_undefined_utils_error() {
    let ret = get_matcher(r"rule: { kind: number, matches: undefined-util }");
    match ret {
      Err(RuleCoreError::Rule(RuleSerializeError::MatchesReference(
        ReferentRuleError::UndefinedUtil(name),
      ))) => {
        assert_eq!(name, "undefined-util");
      }
      _ => panic!("wrong error"),
    }
  }

  #[test]
  fn test_cyclic_transform_error() {
    let ret = get_matcher(
      r"
rule: { kind: number }
transform:
  A: {substring: {source: $B}}
  B: {substring: {source: $A}}",
    );
    assert!(matches!(
      ret,
      Err(RuleCoreError::Transform(TransformError::Cyclic(_)))
    ));
  }

  #[test]
  fn test_rule_reg_with_utils() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore =
      from_str("{rule: {matches: test}, utils: {test: {kind: number}} }").expect("should deser");
    let rule = ReferentRule::try_new("test".into(), &env.registration).expect("should work");
    let not = ReferentRule::try_new("test2".into(), &env.registration).expect("should work");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("a = 123");
    assert!(grep.root().find(&matcher).is_some());
    assert!(grep.root().find(&rule).is_some());
    assert!(grep.root().find(&not).is_none());
    let grep = TypeScript::Tsx.ast_grep("a = '123'");
    assert!(grep.root().find(&matcher).is_none());
    assert!(grep.root().find(&rule).is_none());
    assert!(grep.root().find(&not).is_none());
  }

  #[test]
  fn test_rule_with_constraints() {
    let mut constraints = HashMap::new();
    constraints.insert(
      "A".to_string(),
      Rule::Regex(RegexMatcher::try_new("a").unwrap()),
    );
    let rule =
      RuleCore::new(Rule::Pattern(Pattern::new("$A", TypeScript::Tsx))).with_matchers(constraints);
    let grep = TypeScript::Tsx.ast_grep("a");
    assert!(grep.root().find(&rule).is_some());
    let grep = TypeScript::Tsx.ast_grep("bbb");
    assert!(grep.root().find(&rule).is_none());
  }

  #[test]
  fn test_constraints_inheriting_env() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore =
      from_str("{rule: {pattern: $A = $B}, constraints: {A: {pattern: $B}} }")
        .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("a = a");
    assert!(grep.root().find(&matcher).is_some());
    let grep = TypeScript::Tsx.ast_grep("a = b");
    assert!(grep.root().find(&matcher).is_none());
  }

  #[test]
  fn test_constraints_writing_to_env() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore =
      from_str("{rule: {pattern: $A = $B}, constraints: {B: {pattern: $C + $D}} }")
        .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("a = a");
    assert!(grep.root().find(&matcher).is_none());
    let grep = TypeScript::Tsx.ast_grep("a = 1 + 2");
    let nm = grep.root().find(&matcher).expect("should match");
    let env = nm.get_env();
    let matched = env.get_match("C").expect("should match C").text();
    assert_eq!(matched, "1");
    let matched = env.get_match("D").expect("should match D").text();
    assert_eq!(matched, "2");
  }

  fn get_rewriters() -> (&'static str, RuleCore<TypeScript>) {
    // NOTE: initialize a DeserializeEnv here is not 100% correct
    // it does not inherit global rules or local rules
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let rewriter: SerializableRuleCore =
      from_str("{rule: {kind: number, pattern: $REWRITE}, fix: yjsnp}").expect("should parse");
    let rewriter = rewriter.get_matcher(env).expect("should work");
    ("re", rewriter)
  }

  #[test]
  fn test_rewriter_writing_to_env() {
    let (id, rewriter) = get_rewriters();
    let env = DeserializeEnv::new(TypeScript::Tsx);
    env.registration.insert_rewriter(id, rewriter);
    let ser_rule: SerializableRuleCore = from_str(
      r"
rule: {pattern: $A = $B}
transform:
  C:
    rewrite:
      source: $B
      rewriters: [re]",
    )
    .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("a = 1 + 2");
    let nm = grep.root().find(&matcher).expect("should match");
    let env = nm.get_env();
    let matched = env.get_match("B").expect("should match").text();
    assert_eq!(matched, "1 + 2");
    let matched = env.get_match("A").expect("should match").text();
    assert_eq!(matched, "a");
    let transformed = env.get_transformed("C").expect("should transform");
    assert_eq!(String::from_utf8_lossy(transformed), "yjsnp + yjsnp");
    assert!(env.get_match("REWRITE").is_none());

    let grep = TypeScript::Tsx.ast_grep("a = a");
    let nm = grep.root().find(&matcher).expect("should match");
    let env = nm.get_env();
    let matched = env.get_match("B").expect("should match").text();
    assert_eq!(matched, "a");
    let transformed = env.get_transformed("C").expect("should transform");
    assert_eq!(String::from_utf8_lossy(transformed), "a");
  }
}
