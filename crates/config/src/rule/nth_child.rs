use super::{Rule, RuleSerializeError, SerializableRule};

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Matcher, Node};

use std::borrow::Cow;
use std::collections::HashSet;

use bit_set::BitSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// TODO
#[derive(Debug, Error)]
pub enum NthChildError {}

/// A string or number describing the indices of matching nodes in a list of siblings.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum NthChildSimple {
  /// A number indicating the precise element index
  Numeric(usize),
  /// Functional notation like CSS's An + B
  Functional(String),
}

/// `nthChild` accepts either a number, a string or an object.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged, rename_all = "camelCase")]
pub enum SerializableNthChild {
  Simple(NthChildSimple),
  // TODO add comments
  Complex {
    position: NthChildSimple,
    /// select the nth node that matches the rule, like CSS's of syntax
    of_rule: Option<Box<SerializableRule>>,
    /// matches from the end instead like CSS's nth-last-child
    #[serde(default)]
    reverse: bool,
  },
}

/// Corresponds to the CSS syntax An+B
/// See https://developer.mozilla.org/en-US/docs/Web/CSS/:nth-child#functional_notation
struct FunctionalPosition {
  step_size: usize,
  offset: usize,
}

pub struct NthChild<L: Language> {
  position: FunctionalPosition,
  of_rule: Option<Box<Rule<L>>>,
  reverse: bool,
}

impl<L: Language> NthChild<L> {
  pub fn defined_vars(&self) -> HashSet<&str> {
    todo!()
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    todo!()
  }
}

impl<L: Language> Matcher<L> for NthChild<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    todo!()
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }
}
