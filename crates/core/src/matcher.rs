//! This module defines the core `Matcher` trait in ast-grep.
//!
//! `Matcher` has three notable implementations in this module:
//! * Pattern: matches against a tree-sitter node based on its tree structure.
//! * KindMatcher: matches a node based on its `kind`
//! * RegexMatcher: matches a node based on its textual content using regex.

mod kind;
mod node_match;
mod pattern;
mod text;

use crate::Doc;
use crate::{meta_var::MetaVarEnv, Node};

use bit_set::BitSet;
use std::borrow::Cow;

pub use kind::{kind_utils, KindMatcher, KindMatcherError};
pub use node_match::NodeMatch;
pub use pattern::{Pattern, PatternBuilder, PatternError, PatternNode};
pub use text::{RegexMatcher, RegexMatcherError};

/// `Matcher` defines whether a tree-sitter node matches certain pattern,
/// and update the matched meta-variable values in `MetaVarEnv`.
/// N.B. At least one positive term is required for matching
pub trait Matcher {
  /// Returns the node why the input is matched or None if not matched.
  /// The return value is usually input node itself, but it can be different node.
  /// For example `Has` matcher can return the child or descendant node.
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    _node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>>;

  /// Returns a bitset for all possible target node kind ids.
  /// Returns None if the matcher needs to try against all node kind.
  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }

  /// get_match_len will skip trailing anonymous child node to exclude punctuation.
  // This is not included in NodeMatch since it is only used in replace
  fn get_match_len<D: Doc>(&self, _node: Node<'_, D>) -> Option<usize> {
    None
  }
}

/// MatcherExt provides additional utility methods for `Matcher`.
/// It is implemented for all types that implement `Matcher`.
/// N.B. This trait is not intended to be implemented by users.
pub trait MatcherExt: Matcher {
  fn match_node<'tree, D: Doc>(&self, node: Node<'tree, D>) -> Option<NodeMatch<'tree, D>> {
    // in future we might need to customize initial MetaVarEnv
    let mut env = Cow::Owned(MetaVarEnv::new());
    let node = self.match_node_with_env(node, &mut env)?;
    Some(NodeMatch::new(node, env.into_owned()))
  }

  fn find_node<'tree, D: Doc>(&self, node: Node<'tree, D>) -> Option<NodeMatch<'tree, D>> {
    for n in node.dfs() {
      if let Some(ret) = self.match_node(n.clone()) {
        return Some(ret);
      }
    }
    None
  }
}

impl<T> MatcherExt for T where T: Matcher {}

impl Matcher for str {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let pattern = Pattern::new(self, node.lang().clone());
    pattern.match_node_with_env(node, env)
  }

  fn get_match_len<D: Doc>(&self, node: Node<'_, D>) -> Option<usize> {
    let pattern = Pattern::new(self, node.lang().clone());
    pattern.get_match_len(node)
  }
}

impl<T> Matcher for &T
where
  T: Matcher + ?Sized,
{
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    (**self).match_node_with_env(node, env)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    (**self).potential_kinds()
  }

  fn get_match_len<D: Doc>(&self, node: Node<'_, D>) -> Option<usize> {
    (**self).get_match_len(node)
  }
}

pub struct MatchAll;
impl Matcher for MatchAll {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    Some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    // return None to match anything
    None
  }
}

pub struct MatchNone;
impl Matcher for MatchNone {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    _node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    None
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    // matches nothing
    Some(BitSet::new())
  }
}
