//! This module defines the core `Matcher` trait in ast-grep.
//!
//! `Matcher` has three notable implementations in this module:
//! * Pattern: matches against a tree-sitter node based on its tree structure.
//! * KindMatcher: matches a node based on its `kind`
//! * RegexMatcher: matches a node based on its textual content using regex.

mod kind;
mod node_match;
mod pattern;
#[cfg(feature = "regex")]
mod text;

use crate::meta_var::{MetaVarEnv, SgMetaVarEnv};
use crate::node::SgNode;
use crate::traversal::Pre;
use crate::{Doc, Node};

use bit_set::BitSet;
use std::borrow::Cow;

pub use kind::{kind_utils, KindMatcher, KindMatcherError};
pub use node_match::{NodeMatch, SgNodeMatch};
pub use pattern::{Pattern, PatternError, PatternNode};
#[cfg(feature = "regex")]
pub use text::{RegexMatcher, RegexMatcherError};

/// `Matcher` defines whether a tree-sitter node matches certain pattern,
/// and update the matched meta-variable values in `MetaVarEnv`.
/// N.B. At least one positive term is required for matching
pub trait Matcher {
  /// Returns the node why the input is matched or None if not matched.
  /// The return value is usually input node itself, but it can be different node.
  /// For example `Has` matcher can return the child or descendant node.
  fn match_node_with_env<'tree, N: SgNode<'tree>>(
    &self,
    _node: N,
    _env: &mut Cow<SgMetaVarEnv<'tree, N>>,
  ) -> Option<N>;

  /// Returns a bitset for all possible target node kind ids.
  /// Returns None if the matcher needs to try against all node kind.
  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }

  /// get_match_len will skip trailing anonymous child node to exclude punctuation.
  // This is not included in NodeMatch since it is only used in replace
  fn get_match_len<'tree, N: SgNode<'tree>>(&self, _node: N) -> Option<usize> {
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
  fn match_node_with_env<'tree, N: SgNode<'tree>>(
    &self,
    node: N,
    env: &mut Cow<SgMetaVarEnv<'tree, N>>,
  ) -> Option<N> {
    let pattern = Pattern::str(self, node.lang().clone());
    pattern.match_node_with_env(node, env)
  }

  fn get_match_len<'tree, N: SgNode<'tree>>(&self, node: N) -> Option<usize> {
    let pattern = Pattern::str(self, node.lang().clone());
    pattern.get_match_len(node)
  }
}

impl<T> Matcher for &T
where
  T: Matcher + ?Sized,
{
  fn match_node_with_env<'tree, N: SgNode<'tree>>(
    &self,
    node: N,
    env: &mut Cow<SgMetaVarEnv<'tree, N>>,
  ) -> Option<N> {
    (**self).match_node_with_env(node, env)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    (**self).potential_kinds()
  }

  fn get_match_len<'tree, N: SgNode<'tree>>(&self, node: N) -> Option<usize> {
    (**self).get_match_len(node)
  }
}

pub struct FindAllNodes<'tree, D: Doc, M: Matcher> {
  // using dfs is not universally correct, say, when we want replace nested matches
  // e.g. for pattern Some($A) with replacement $A, Some(Some(1)) will cause panic
  dfs: Pre<'tree, D>,
  matcher: M,
}

impl<'tree, D: Doc, M: Matcher> FindAllNodes<'tree, D, M> {
  pub fn new(matcher: M, node: Node<'tree, D>) -> Self {
    Self {
      dfs: node.dfs(),
      matcher,
    }
  }
}

impl<'tree, D: Doc, M: Matcher> Iterator for FindAllNodes<'tree, D, M> {
  type Item = NodeMatch<'tree, D>;
  fn next(&mut self) -> Option<Self::Item> {
    let kinds = self.matcher.potential_kinds();
    for cand in self.dfs.by_ref() {
      if let Some(k) = &kinds {
        if !k.contains(cand.kind_id().into()) {
          continue;
        }
      }
      if let Some(matched) = self.matcher.match_node(cand) {
        return Some(matched);
      }
    }
    None
  }
}

pub struct MatchAll;
impl Matcher for MatchAll {
  fn match_node_with_env<'tree, N: SgNode<'tree>>(
    &self,
    node: N,
    _env: &mut Cow<SgMetaVarEnv<'tree, N>>,
  ) -> Option<N> {
    Some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    // return None to match anything
    None
  }
}

pub struct MatchNone;
impl Matcher for MatchNone {
  fn match_node_with_env<'tree, N: SgNode<'tree>>(
    &self,
    _node: N,
    _env: &mut Cow<SgMetaVarEnv<'tree, N>>,
  ) -> Option<N> {
    None
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    // matches nothing
    Some(BitSet::new())
  }
}
