use crate::RuleWithConstraint;

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, Weak};

#[derive(Clone)]
pub struct RuleRegistration<L: Language> {
  inner: Arc<RwLock<HashMap<String, RuleWithConstraint<L>>>>,
}

// these are shit code
impl<L: Language> RuleRegistration<L> {
  pub fn get_rules(&self) -> RwLockReadGuard<HashMap<String, RuleWithConstraint<L>>> {
    self.inner.read().unwrap()
  }

  pub fn get_ref(&self) -> RegistrationRef<L> {
    let inner = Arc::downgrade(&self.inner);
    RegistrationRef { inner }
  }

  pub fn insert_rule(
    &self,
    id: String,
    rule: RuleWithConstraint<L>,
  ) -> Result<(), ReferentRuleError> {
    let mut map = self.inner.write().unwrap(); // TODO
    if map.contains_key(&id) {
      return Err(ReferentRuleError::DupicateRule(id));
    }
    map.insert(id, rule);
    Ok(())
  }
}

pub struct RegistrationRef<L: Language> {
  inner: Weak<RwLock<HashMap<String, RuleWithConstraint<L>>>>,
}
// these are shit code
impl<L: Language> RegistrationRef<L> {
  pub fn unref(&self) -> RuleRegistration<L> {
    let inner = self.inner.upgrade().unwrap();
    RuleRegistration { inner }
  }
}

impl<L: Language> Default for RuleRegistration<L> {
  fn default() -> Self {
    Self {
      inner: Default::default(),
    }
  }
}

#[derive(Debug, Error)]
pub enum ReferentRuleError {
  #[error("Rule `{0}` is not found.")]
  RuleNotFound(String),
  #[error("Duplicate rule id `{0}` is found.")]
  DupicateRule(String),
}

pub struct ReferentRule<L: Language> {
  rule_id: String,
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
}

impl<L: Language> Matcher<L> for ReferentRule<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    let registration = self.reg_ref.unref();
    let rules = registration.get_rules();
    let rule = rules.get(&self.rule_id)?;
    rule.match_node_with_env(node, env)
  }
  fn potential_kinds(&self) -> Option<BitSet> {
    let registration = self.reg_ref.unref();
    let rules = registration.get_rules();
    let rule = rules.get(&self.rule_id)?;
    rule.potential_kinds()
  }
}
