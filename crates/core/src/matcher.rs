mod kind;
mod node_match;

use crate::meta_var::{MetaVarEnv, MetaVarMatchers};
use crate::node::Dfs;
use crate::Language;
use crate::Node;
use crate::Pattern;

use bit_set::BitSet;

pub use kind::KindMatcher;
pub use node_match::NodeMatch;

/**
 * N.B. At least one positive term is required for matching
 */
pub trait Matcher<L: Language> {
  /// Returns the node why the input is matched or None if not matched.
  /// The return value is usually input node itself, but it can be different node.
  /// For example `Has` matcher can return the child or descendant node.
  fn match_node_with_env<'tree>(
    &self,
    _node: Node<'tree, L>,
    _env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>>;

  /// Returns a bitset for all possible target node kind ids.
  /// Returns None if the matcher needs to try against all node kind.
  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }

  /// get_match_len will skip trailing anonymous child node to exclude punctuation.
  // This is not included in NodeMatch since it is only used in replace
  fn get_match_len(&self, _node: Node<L>) -> Option<usize> {
    None
  }

  fn match_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    let mut env = self.get_meta_var_env();
    let node = self.match_node_with_env(node, &mut env)?;
    env.match_constraints().then_some(NodeMatch::new(node, env))
  }

  fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
    MetaVarMatchers::new()
  }

  fn get_meta_var_env<'tree>(&self) -> MetaVarEnv<'tree, L> {
    MetaVarEnv::from_matchers(self.get_meta_var_matchers())
  }

  fn find_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    let node = self.match_node_with_env(node.clone(), env).or_else(|| {
      node
        .children()
        .find_map(|sub| self.find_node_with_env(sub, env))
    })?;
    env.match_constraints().then_some(node)
  }

  fn find_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    if let Some(set) = self.potential_kinds() {
      return find_node_impl(self, node, &set);
    }
    self
      .match_node(node.clone())
      .or_else(|| node.children().find_map(|sub| self.find_node(sub)))
  }

  fn find_all_nodes(self, node: Node<L>) -> FindAllNodes<L, Self>
  where
    Self: Sized,
  {
    FindAllNodes::new(self, node)
  }
}

fn find_node_impl<'tree, L, M>(
  m: &M,
  node: Node<'tree, L>,
  set: &BitSet,
) -> Option<NodeMatch<'tree, L>>
where
  L: Language,
  M: Matcher<L> + ?Sized,
{
  for n in node.dfs() {
    if set.contains(n.kind_id().into()) {
      if let Some(ret) = m.match_node(n.clone()) {
        return Some(ret);
      }
    }
  }
  None
}

impl<L: Language> Matcher<L> for str {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    let pattern = Pattern::new(self, node.root.lang.clone());
    pattern.match_node_with_env(node, env)
  }

  fn get_match_len(&self, node: Node<L>) -> Option<usize> {
    let pattern = Pattern::new(self, node.lang().clone());
    pattern.get_match_len(node)
  }
}

impl<L, T> Matcher<L> for &T
where
  L: Language,
  T: Matcher<L> + ?Sized,
{
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    (**self).match_node_with_env(node, env)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    (**self).potential_kinds()
  }

  fn match_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    (**self).match_node(node)
  }

  fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
    (**self).get_meta_var_matchers()
  }

  fn get_meta_var_env<'tree>(&self) -> MetaVarEnv<'tree, L> {
    (**self).get_meta_var_env()
  }

  fn find_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    (**self).find_node_with_env(node, env)
  }

  fn find_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    (**self).find_node(node)
  }

  fn get_match_len(&self, node: Node<L>) -> Option<usize> {
    (**self).get_match_len(node)
  }
}

impl<L: Language> Matcher<L> for Box<dyn Matcher<L>> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    // NOTE: must double deref boxed value to avoid recursion
    (**self).match_node_with_env(node, env)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    (**self).potential_kinds()
  }

  fn match_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    (**self).match_node(node)
  }

  fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
    (**self).get_meta_var_matchers()
  }

  fn get_meta_var_env<'tree>(&self) -> MetaVarEnv<'tree, L> {
    (**self).get_meta_var_env()
  }

  fn find_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    (**self).find_node_with_env(node, env)
  }

  fn find_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    (**self).find_node(node)
  }

  fn get_match_len(&self, node: Node<L>) -> Option<usize> {
    (**self).get_match_len(node)
  }
}

pub struct FindAllNodes<'tree, L: Language, M: Matcher<L>> {
  // using dfs is not universally correct, say, when we want replace nested matches
  // e.g. for pattern Some($A) with replacement $A, Some(Some(1)) will cause panic
  dfs: Dfs<'tree, L>,
  matcher: M,
}

impl<'tree, L: Language, M: Matcher<L>> FindAllNodes<'tree, L, M> {
  fn new(matcher: M, node: Node<'tree, L>) -> Self {
    Self {
      dfs: node.dfs(),
      matcher,
    }
  }
}

impl<'tree, L: Language, M: Matcher<L>> Iterator for FindAllNodes<'tree, L, M> {
  type Item = NodeMatch<'tree, L>;
  fn next(&mut self) -> Option<Self::Item> {
    for cand in self.dfs.by_ref() {
      if let Some(matched) = self.matcher.match_node(cand) {
        return Some(matched);
      }
    }
    None
  }
}

pub struct MatchAll;
impl<L: Language> Matcher<L> for MatchAll {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    _env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    Some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    // return None to match anything
    None
  }
}

pub struct MatchNone;
impl<L: Language> Matcher<L> for MatchNone {
  fn match_node_with_env<'tree>(
    &self,
    _node: Node<'tree, L>,
    _env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    None
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    // matches nothing
    Some(BitSet::new())
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::Root;

  fn pattern_node(s: &str) -> Root<Tsx> {
    Root::new(s, Tsx)
  }

  #[test]
  fn test_box_match() {
    let boxed: Box<dyn Matcher<Tsx>> = Box::new("const a = 123");
    let cand = pattern_node("const a = 123");
    let cand = cand.root();
    assert!(boxed.find_node(cand).is_some());
  }
}
