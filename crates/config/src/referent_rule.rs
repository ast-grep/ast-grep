use crate::RuleWithConstraint;

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct RuleRegistration<L: Language> {
  inner: Arc<RwLock<HashMap<String, RuleWithConstraint<L>>>>,
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
}

pub struct ReferentRule<L: Language> {
  rule_id: String,
  // TODO: this is WRONG! we should use weak ref here
  registration: RuleRegistration<L>,
}

impl<L: Language> ReferentRule<L> {
  pub fn try_new(
    rule_id: String,
    registration: RuleRegistration<L>,
  ) -> Result<Self, ReferentRuleError> {
    Ok(Self {
      registration,
      rule_id,
    })
  }
}

impl<L: Language> Matcher<L> for ReferentRule<L> {
  fn match_node_with_env<'tree>(
    &self,
    _node: Node<'tree, L>,
    _env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    todo!()
  }
  fn potential_kinds(&self) -> Option<BitSet> {
    todo!()
  }
}
