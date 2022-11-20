use crate::meta_var::{MetaVarEnv, MetaVarMatchers};
use crate::node::{Dfs, KindId};
use crate::replacer::Replacer;
use crate::ts_parser::Edit;
use crate::Language;
use crate::Node;
use crate::Pattern;
use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Clone)]
pub struct KindMatcher<L: Language> {
  kind: KindId,
  lang: PhantomData<L>,
}

// 0 is symbol_end for not found, 65535 is builtin symbol ERROR
// see https://tree-sitter.docsforge.com/master/api/#TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION
// and https://tree-sitter.docsforge.com/master/api/ts_language_symbol_for_name/
const TS_BUILTIN_SYM_END: KindId = 0;
const TS_BUILTIN_SYM_ERROR: KindId = 65535;

impl<L: Language> KindMatcher<L> {
  pub fn new(node_kind: &str, lang: L) -> Self {
    Self {
      kind: lang
        .get_ts_language()
        .id_for_node_kind(node_kind, /*named*/ true),
      lang: PhantomData,
    }
  }

  pub fn from_id(kind: KindId) -> Self {
    Self {
      kind,
      lang: PhantomData,
    }
  }

  /// Whether the kind matcher contains undefined tree-sitter kind.
  pub fn is_invalid(&self) -> bool {
    self.kind == TS_BUILTIN_SYM_END
  }

  /// Whether the kind will match parsing error occurred in the source code.
  /// for example, we can use `kind: ERROR` in YAML to find invalid syntax in source.
  /// the name `is_error` implies the matcher itself is error.
  /// But here the matcher itself is valid and it is what it matches is error.
  pub fn is_error_matcher(&self) -> bool {
    self.kind == TS_BUILTIN_SYM_ERROR
  }
}

impl<L: Language> Matcher<L> for KindMatcher<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    _env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    if node.kind_id() == self.kind {
      Some(node)
    } else {
      None
    }
  }
}

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

  // get_match_len will skip trailing anonymous child node to exclude punctuation.
  // This is not included in NodeMatch since it is only used in replace
  fn get_match_len(&self, _node: Node<L>) -> Option<usize> {
    None
  }

  fn match_node<'tree>(&self, node: Node<'tree, L>) -> Option<NodeMatch<'tree, L>> {
    let mut env = self.get_meta_var_env();
    let node = self.match_node_with_env(node, &mut env)?;
    env.match_constraints().then_some(NodeMatch(node, env))
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
}

pub struct FindAllNodes<'tree, L: Language, M: Matcher<L>> {
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
}

pub struct NodeMatch<'tree, L: Language>(Node<'tree, L>, MetaVarEnv<'tree, L>);

impl<'tree, L: Language> NodeMatch<'tree, L> {
  pub fn get_node(&self) -> &Node<'tree, L> {
    &self.0
  }

  pub fn get_env(&self) -> &MetaVarEnv<'tree, L> {
    &self.1
  }

  pub fn replace_by<R: Replacer<L>>(&self, replacer: R) -> Edit {
    let lang = self.lang().clone();
    let env = self.get_env();
    let range = self.range();
    let position = range.start;
    let deleted_length = range.len();
    let inserted_text = replacer.generate_replacement(env, lang);
    Edit {
      position,
      deleted_length,
      inserted_text,
    }
  }
}

impl<'tree, L: Language> From<Node<'tree, L>> for NodeMatch<'tree, L> {
  fn from(node: Node<'tree, L>) -> Self {
    Self(node, MetaVarEnv::new())
  }
}

impl<'tree, L: Language> From<NodeMatch<'tree, L>> for Node<'tree, L> {
  fn from(node_match: NodeMatch<'tree, L>) -> Self {
    node_match.0
  }
}

impl<'tree, L: Language> Deref for NodeMatch<'tree, L> {
  type Target = Node<'tree, L>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl<'tree, L: Language> DerefMut for NodeMatch<'tree, L> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
impl<'tree, L: Language> Borrow<Node<'tree, L>> for NodeMatch<'tree, L> {
  fn borrow(&self) -> &Node<'tree, L> {
    &self.0
  }
}
impl<'tree, L: Language> BorrowMut<Node<'tree, L>> for NodeMatch<'tree, L> {
  fn borrow_mut(&mut self) -> &mut Node<'tree, L> {
    &mut self.0
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
  fn test_kind_match() {
    let kind = "public_field_definition";
    let cand = pattern_node("class A { a = 123 }");
    let cand = cand.root();
    let pattern = KindMatcher::new(kind, Tsx);
    assert!(
      pattern.find_node(cand.clone()).is_some(),
      "goal: {}, candidate: {}",
      kind,
      cand.inner.to_sexp(),
    );
  }

  #[test]
  fn test_kind_non_match() {
    let kind = "field_definition";
    let cand = pattern_node("const a = 123");
    let cand = cand.root();
    let pattern = KindMatcher::new(kind, Tsx);
    assert!(
      pattern.find_node(cand.clone()).is_none(),
      "goal: {}, candidate: {}",
      kind,
      cand.inner.to_sexp(),
    );
  }

  #[test]
  fn test_box_match() {
    let boxed: Box<dyn Matcher<Tsx>> = Box::new("const a = 123");
    let cand = pattern_node("const a = 123");
    let cand = cand.root();
    assert!(boxed.find_node(cand).is_some());
  }
}
