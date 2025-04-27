use crate::{Rule, RuleCore};

use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

pub struct Registration<R>(Arc<HashMap<String, R>>);

impl<R> Clone for Registration<R> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<R> Registration<R> {
  #[allow(clippy::mut_from_ref)]
  fn write(&self) -> &mut HashMap<String, R> {
    // SAFETY: `write` will only be called during initialization and
    // it only insert new item to the hashmap. It is safe to cast the raw ptr.
    unsafe { &mut *(Arc::as_ptr(&self.0) as *mut HashMap<String, R>) }
  }
}
pub type GlobalRules = Registration<RuleCore>;

impl GlobalRules {
  pub fn insert(&self, id: &str, rule: RuleCore) -> Result<(), ReferentRuleError> {
    let map = self.write();
    if map.contains_key(id) {
      return Err(ReferentRuleError::DuplicateRule(id.into()));
    }
    map.insert(id.to_string(), rule);
    let rule = map.get(id).unwrap();
    // TODO: we can skip check here because insertion order
    // is guaranteed in deserialize_env
    if rule.check_cyclic(id) {
      return Err(ReferentRuleError::CyclicRule(id.to_string()));
    }
    Ok(())
  }
}

impl<R> Default for Registration<R> {
  fn default() -> Self {
    Self(Default::default())
  }
}

#[derive(Clone, Default)]
pub struct RuleRegistration {
  /// utility rule to every RuleCore, every sub-rule has its own local utility
  local: Registration<Rule>,
  /// global rules are shared by all RuleConfigs. It is a singleton.
  global: Registration<RuleCore>,
  /// Every RuleConfig has its own rewriters. But sub-rules share parent's rewriters.
  rewriters: Registration<RuleCore>,
}

// these are shit code
impl RuleRegistration {
  pub fn get_rewriters(&self) -> &HashMap<String, RuleCore> {
    &self.rewriters.0
  }

  pub fn from_globals(global: &GlobalRules) -> Self {
    Self {
      local: Default::default(),
      global: global.clone(),
      rewriters: Default::default(),
    }
  }

  fn get_ref(&self) -> RegistrationRef {
    let local = Arc::downgrade(&self.local.0);
    let global = Arc::downgrade(&self.global.0);
    RegistrationRef { local, global }
  }

  pub(crate) fn insert_local(&self, id: &str, rule: Rule) -> Result<(), ReferentRuleError> {
    let map = self.local.write();
    if map.contains_key(id) {
      return Err(ReferentRuleError::DuplicateRule(id.into()));
    }
    map.insert(id.to_string(), rule);
    let rule = map.get(id).unwrap();
    // TODO: we can skip check here because insertion order
    // is guaranteed in deserialize_env
    if rule.check_cyclic(id) {
      return Err(ReferentRuleError::CyclicRule(id.to_string()));
    }
    Ok(())
  }

  pub(crate) fn insert_rewriter(&self, id: &str, rewriter: RuleCore) {
    self.rewriters.insert(id, rewriter).expect("should work");
  }

  pub(crate) fn get_local_util_vars(&self) -> HashSet<&str> {
    let mut ret = HashSet::new();
    let utils = &self.local.0;
    for rule in utils.values() {
      for v in rule.defined_vars() {
        ret.insert(v);
      }
    }
    ret
  }
}

/// RegistrationRef must use Weak pointer to avoid
/// cyclic reference in RuleRegistration
struct RegistrationRef {
  local: Weak<HashMap<String, Rule>>,
  global: Weak<HashMap<String, RuleCore>>,
}
impl RegistrationRef {
  fn get_local(&self) -> Arc<HashMap<String, Rule>> {
    self
      .local
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
  fn get_global(&self) -> Arc<HashMap<String, RuleCore>> {
    self
      .global
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
}

#[derive(Debug, Error)]
pub enum ReferentRuleError {
  #[error("Rule `{0}` is not defined.")]
  UndefinedUtil(String),
  #[error("Duplicate rule id `{0}` is found.")]
  DuplicateRule(String),
  #[error("Rule `{0}` has a cyclic dependency in its `matches` sub-rule.")]
  CyclicRule(String),
}

pub struct ReferentRule {
  pub(crate) rule_id: String,
  reg_ref: RegistrationRef,
}

impl ReferentRule {
  pub fn try_new(
    rule_id: String,
    registration: &RuleRegistration,
  ) -> Result<Self, ReferentRuleError> {
    Ok(Self {
      reg_ref: registration.get_ref(),
      rule_id,
    })
  }

  fn eval_local<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&Rule) -> T,
  {
    let rules = self.reg_ref.get_local();
    let rule = rules.get(&self.rule_id)?;
    Some(func(rule))
  }

  fn eval_global<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&RuleCore) -> T,
  {
    let rules = self.reg_ref.get_global();
    let rule = rules.get(&self.rule_id)?;
    Some(func(rule))
  }

  pub(super) fn verify_util(&self) -> Result<(), ReferentRuleError> {
    let rules = self.reg_ref.get_local();
    if rules.contains_key(&self.rule_id) {
      return Ok(());
    }
    let rules = self.reg_ref.get_global();
    if rules.contains_key(&self.rule_id) {
      return Ok(());
    }
    Err(ReferentRuleError::UndefinedUtil(self.rule_id.clone()))
  }
}

impl Matcher for ReferentRule {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    self
      .eval_local(|r| r.match_node_with_env(node.clone(), env))
      .or_else(|| self.eval_global(|r| r.match_node_with_env(node, env)))
      .flatten()
  }
  fn potential_kinds(&self) -> Option<BitSet> {
    self
      .eval_local(|r| {
        debug_assert!(!r.check_cyclic(&self.rule_id), "no cyclic rule allowed");
        r.potential_kinds()
      })
      .or_else(|| {
        self.eval_global(|r| {
          debug_assert!(!r.check_cyclic(&self.rule_id), "no cyclic rule allowed");
          r.potential_kinds()
        })
      })
      .flatten()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::rule::Rule;
  use crate::test::TypeScript as TS;
  use ast_grep_core::ops as o;
  use ast_grep_core::Pattern;

  type Result = std::result::Result<(), ReferentRuleError>;

  #[test]
  fn test_cyclic_error() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::Matches(rule);
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule(_))));
    Ok(())
  }

  #[test]
  fn test_cyclic_all() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::All(o::All::new(std::iter::once(Rule::Matches(rule))));
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule(_))));
    Ok(())
  }

  #[test]
  fn test_cyclic_not() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::Not(Box::new(o::Not::new(Rule::Matches(rule))));
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule(_))));
    Ok(())
  }

  #[test]
  fn test_success_rule() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let pattern = Rule::Pattern(Pattern::new("some", TS::Tsx));
    let ret = registration.insert_local("test", pattern);
    assert!(ret.is_ok());
    assert!(rule.potential_kinds().is_some());
    Ok(())
  }
}
