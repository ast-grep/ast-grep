use super::Matcher;
use crate::meta_var::MetaVarEnv;
use crate::replacer::Replacer;
use crate::source::Edit;
use crate::{Doc, Node};

use std::borrow::Borrow;
use std::ops::Deref;

/// Represents the matched node with populated MetaVarEnv.
/// It derefs to the Node so you can use it as a Node.
/// To access the underlying MetaVarEnv, call `get_env` method.
#[derive(Clone)]
pub struct NodeMatch<'tree, D: Doc>(Node<'tree, D>, MetaVarEnv<'tree, D>);

impl<'tree, D: Doc> NodeMatch<'tree, D> {
  pub fn new(node: Node<'tree, D>, env: MetaVarEnv<'tree, D>) -> Self {
    Self(node, env)
  }

  pub fn get_node(&self) -> &Node<'tree, D> {
    &self.0
  }

  /// Returns the populated MetaVarEnv for this match.
  pub fn get_env(&self) -> &MetaVarEnv<'tree, D> {
    &self.1
  }
  pub fn get_env_mut(&mut self) -> &mut MetaVarEnv<'tree, D> {
    &mut self.1
  }
  /// # Safety
  /// should only called for readopting nodes
  pub(crate) unsafe fn get_node_mut(&mut self) -> &mut Node<'tree, D> {
    &mut self.0
  }
}

impl<'tree, D: Doc> NodeMatch<'tree, D> {
  pub fn replace_by<R: Replacer<D>>(&self, replacer: R) -> Edit<D::Source> {
    let range = self.range();
    let position = range.start;
    let deleted_length = range.len();
    let inserted_text = replacer.generate_replacement(self);
    Edit {
      position,
      deleted_length,
      inserted_text,
    }
  }

  #[doc(hidden)]
  pub fn make_edit<M, R>(&self, matcher: &M, replacer: &R) -> Edit<D::Source>
  where
    M: Matcher<D::Lang>,
    R: Replacer<D>,
  {
    let range = self.range();
    let position = range.start;
    let deleted_length = matcher
      .get_match_len(self.get_node().clone())
      .unwrap_or_else(|| range.len());
    let inserted_text = replacer.generate_replacement(self);
    Edit {
      position,
      deleted_length,
      inserted_text,
    }
  }
}

impl<'tree, D: Doc> From<Node<'tree, D>> for NodeMatch<'tree, D> {
  fn from(node: Node<'tree, D>) -> Self {
    Self(node, MetaVarEnv::new())
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, D: Doc> From<NodeMatch<'tree, D>> for Node<'tree, D> {
  fn from(node_match: NodeMatch<'tree, D>) -> Self {
    node_match.0
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, D: Doc> Deref for NodeMatch<'tree, D> {
  type Target = Node<'tree, D>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, D: Doc> Borrow<Node<'tree, D>> for NodeMatch<'tree, D> {
  fn borrow(&self) -> &Node<'tree, D> {
    &self.0
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::{Language, StrDoc};

  fn use_node<L: Language>(n: &Node<StrDoc<L>>) -> String {
    n.text().to_string()
  }

  fn borrow_node<'a, D, B>(b: B) -> String
  where
    D: Doc + 'static,
    B: Borrow<Node<'a, D>>,
  {
    b.borrow().text().to_string()
  }

  #[test]
  fn test_node_match_as_node() {
    let root = Tsx.ast_grep("var a = 1");
    let node = root.root();
    let src = node.text().to_string();
    let nm = NodeMatch::from(node);
    let ret = use_node(&*nm);
    assert_eq!(ret, src);
    assert_eq!(use_node(&*nm), borrow_node(nm));
  }

  #[test]
  fn test_node_env() {
    let root = Tsx.ast_grep("var a = 1");
    let find = root.root().find("var $A = 1").expect("should find");
    let env = find.get_env();
    let node = env.get_match("A").expect("should find");
    assert_eq!(node.text(), "a");
  }

  #[test]
  fn test_replace_by() {
    let root = Tsx.ast_grep("var a = 1");
    let find = root.root().find("var $A = 1").expect("should find");
    let fixed = find.replace_by("var b = $A");
    assert_eq!(fixed.position, 0);
    assert_eq!(fixed.deleted_length, 9);
    assert_eq!(fixed.inserted_text, "var b = a".as_bytes());
  }
}
