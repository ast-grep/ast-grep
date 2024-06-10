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
  step_size: i32,
  offset: i32,
}

impl FunctionalPosition {
  /// index is 0-based, but output is 1-based
  fn is_matched(&self, index: usize) -> bool {
    let index = (index + 1) as i32;
    let FunctionalPosition { step_size, offset } = self;
    if *step_size == 0 {
      index == *offset
    } else {
      let n = index - offset;
      n / step_size >= 0 && n % step_size == 0
    }
  }
}

pub struct NthChild<L: Language> {
  position: FunctionalPosition,
  of_rule: Option<Box<Rule<L>>>,
  reverse: bool,
}

impl<L: Language> NthChild<L> {
  fn find_index<'t, D: Doc<Lang = L>>(
    &self,
    node: &Node<'t, D>,
    env: &mut Cow<MetaVarEnv<'t, D>>,
  ) -> Option<usize> {
    let parent = node.parent()?;
    let mut children: Vec<_> = if let Some(rule) = &self.of_rule {
      parent
        .children()
        .filter(|n| n.is_named())
        .filter_map(|child| rule.match_node_with_env(child, env))
        .collect()
    } else {
      parent.children().filter(|n| n.is_named()).collect()
    };
    if self.reverse {
      children.reverse()
    }
    children
      .iter()
      .position(|child| child.node_id() == node.node_id())
  }
  pub fn defined_vars(&self) -> HashSet<&str> {
    if let Some(rule) = &self.of_rule {
      rule.defined_vars()
    } else {
      HashSet::new()
    }
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    if let Some(rule) = &self.of_rule {
      rule.verify_util()
    } else {
      Ok(())
    }
  }
}

impl<L: Language> Matcher<L> for NthChild<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let index = self.find_index(&node, env)?;
    self.position.is_matched(index).then_some(node)
  }
  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript as TS;

  #[test]
  fn test_positional() {
    let position = FunctionalPosition {
      step_size: 0,
      offset: 1,
    };
    assert!(position.is_matched(0));
    assert!(!position.is_matched(1));
    assert!(!position.is_matched(2));
  }

  #[test]
  fn test_positional_an_b() {
    let position = FunctionalPosition {
      step_size: 2,
      offset: -1,
    };
    assert!(position.is_matched(0));
    assert!(!position.is_matched(1));
    assert!(position.is_matched(2));
    assert!(!position.is_matched(3));
    assert!(position.is_matched(4));
  }

  #[test]
  fn test_find_index() {
    let mut env = Cow::Owned(MetaVarEnv::new());
    let rule = NthChild {
      position: FunctionalPosition {
        step_size: 2,
        offset: -1,
      },
      of_rule: None,
      reverse: false,
    };
    let grep = TS::Tsx.ast_grep("[1,2,3,4]");
    let node = grep.root().find("1").unwrap();
    assert_eq!(rule.find_index(&node, &mut env), Some(0));
  }
}
