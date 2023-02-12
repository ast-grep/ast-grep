use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::marker::PhantomData;

#[derive(Debug, Error)]
pub enum ReferentRuleError {}

pub struct ReferentRule<L: Language> {
  lang: PhantomData<L>,
  rule_id: String,
}

impl<L: Language> ReferentRule<L> {
  pub fn try_new(rule_id: String) -> Result<Self, ReferentRuleError> {
    Ok(Self {
      lang: PhantomData,
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
