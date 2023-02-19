use crate::{Rule, RuleWithConstraint};

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, Weak};

#[derive(Clone)]
pub struct RuleRegistration<L: Language> {
  local: Arc<RwLock<HashMap<String, Rule<L>>>>,
  global: Arc<RwLock<HashMap<String, RuleWithConstraint<L>>>>,
}

// these are shit code
impl<L: Language> RuleRegistration<L> {
  fn get_local(&self) -> RwLockReadGuard<HashMap<String, Rule<L>>> {
    self.local.read().unwrap()
  }

  fn get_global(&self) -> RwLockReadGuard<HashMap<String, RuleWithConstraint<L>>> {
    self.global.read().unwrap()
  }

  pub fn get_ref(&self) -> RegistrationRef<L> {
    let local = Arc::downgrade(&self.local);
    let global = Arc::downgrade(&self.global);
    RegistrationRef { local, global }
  }

  pub fn insert_local(&self, id: &str, rule: Rule<L>) -> Result<(), ReferentRuleError> {
    let mut map = self.local.write().unwrap(); // TODO
    if map.contains_key(id) {
      return Err(ReferentRuleError::DupicateRule(id.into()));
    }
    map.insert(id.to_string(), rule);
    let rule = map.get(id).unwrap();
    if rule.check_cyclic(id) {
      return Err(ReferentRuleError::CyclicRule);
    }
    Ok(())
  }

  pub fn insert_global(
    &self,
    id: &str,
    rule: RuleWithConstraint<L>,
  ) -> Result<(), ReferentRuleError> {
    let mut map = self.global.write().unwrap(); // TODO
    if map.contains_key(id) {
      return Err(ReferentRuleError::DupicateRule(id.into()));
    }
    map.insert(id.to_string(), rule);
    let rule = map.get(id).unwrap();
    if rule.check_cyclic(id) {
      return Err(ReferentRuleError::CyclicRule);
    }
    Ok(())
  }
}

impl<L: Language> Default for RuleRegistration<L> {
  fn default() -> Self {
    Self {
      local: Default::default(),
      global: Default::default(),
    }
  }
}

pub struct RegistrationRef<L: Language> {
  local: Weak<RwLock<HashMap<String, Rule<L>>>>,
  global: Weak<RwLock<HashMap<String, RuleWithConstraint<L>>>>,
}
// these are shit code
impl<L: Language> RegistrationRef<L> {
  pub fn unref(&self) -> RuleRegistration<L> {
    let local = self.local.upgrade().unwrap();
    let global = self.global.upgrade().unwrap();
    RuleRegistration { local, global }
  }
}

#[derive(Debug, Error)]
pub enum ReferentRuleError {
  #[error("Rule `{0}` is not found.")]
  RuleNotFound(String),
  #[error("Duplicate rule id `{0}` is found.")]
  DupicateRule(String),
  #[error("Rule has a cyclic dependency in its `matches` sub-rule.")]
  CyclicRule,
}

pub struct ReferentRule<L: Language> {
  pub(crate) rule_id: String,
  reg_ref: RegistrationRef<L>,
}

impl<L: Language> ReferentRule<L> {
  pub fn try_new(
    rule_id: String,
    registration: &RuleRegistration<L>,
  ) -> Result<Self, ReferentRuleError> {
    Ok(Self {
      reg_ref: registration.get_ref(),
      rule_id,
    })
  }

  fn eval_local<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&Rule<L>) -> T,
  {
    let registration = self.reg_ref.unref();
    let rules = registration.get_local();
    let rule = rules.get(&self.rule_id)?;
    Some(func(rule))
  }

  fn eval_global<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&RuleWithConstraint<L>) -> T,
  {
    let registration = self.reg_ref.unref();
    let rules = registration.get_global();
    let rule = rules.get(&self.rule_id)?;
    Some(func(rule))
  }
}

impl<L: Language> Matcher<L> for ReferentRule<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
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
    let registration = RuleRegistration::<TS>::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::Matches(rule);
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule)));
    Ok(())
  }

  #[test]
  fn test_cyclic_all() -> Result {
    let registration = RuleRegistration::<TS>::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::All(o::All::new(std::iter::once(Rule::Matches(rule))));
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule)));
    Ok(())
  }

  #[test]
  fn test_success_rule() -> Result {
    let registration = RuleRegistration::<TS>::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let pattern = Rule::Pattern(Pattern::new("some", TS::Tsx));
    let ret = registration.insert_local("test", pattern);
    assert!(ret.is_ok());
    assert!(rule.potential_kinds().is_some());
    Ok(())
  }
}
