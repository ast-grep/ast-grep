use super::Matcher;
use crate::meta_var::SgMetaVarEnv;
use crate::node::SgNode;
use crate::replacer::Replacer;
use crate::source::{Content, Edit};
use crate::{Doc, Node};

use std::borrow::Borrow;
use std::ops::Deref;

/// Represents the matched node with populated MetaVarEnv.
/// It derefs to the Node so you can use it as a Node.
/// To access the underlying MetaVarEnv, call `get_env` method.
#[derive(Clone)]
pub struct SgNodeMatch<
  't,
  N: SgNode<'t>,
  C = Vec<<<<N as SgNode<'t>>::Doc as Doc>::Source as Content>::Underlying>,
>(N, SgMetaVarEnv<'t, N, C>);
pub type NodeMatch<'t, D> =
  SgNodeMatch<'t, Node<'t, D>, Vec<<<D as Doc>::Source as Content>::Underlying>>;

impl<'tree, N: SgNode<'tree>, C> SgNodeMatch<'tree, N, C> {
  pub fn new(node: N, env: SgMetaVarEnv<'tree, N, C>) -> Self {
    Self(node, env)
  }

  pub fn get_node(&self) -> &N {
    &self.0
  }

  /// Returns the populated MetaVarEnv for this match.
  pub fn get_env(&self) -> &SgMetaVarEnv<'tree, N, C> {
    &self.1
  }
  pub fn get_env_mut(&mut self) -> &mut SgMetaVarEnv<'tree, N, C> {
    &mut self.1
  }
  /// # Safety
  /// should only called for readopting nodes
  pub(crate) unsafe fn get_node_mut(&mut self) -> &mut N {
    &mut self.0
  }
}

impl<D: Doc> NodeMatch<'_, D> {
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
    M: Matcher,
    R: Replacer<D>,
  {
    let range = replacer.get_replaced_range(self, matcher);
    let inserted_text = replacer.generate_replacement(self);
    Edit {
      position: range.start,
      deleted_length: range.len(),
      inserted_text,
    }
  }
}

impl<'tree, N: SgNode<'tree>, C> From<N> for SgNodeMatch<'tree, N, C> {
  fn from(node: N) -> Self {
    Self(node, SgMetaVarEnv::new())
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, D: Doc> From<NodeMatch<'tree, D>> for Node<'tree, D> {
  fn from(node_match: NodeMatch<'tree, D>) -> Self {
    node_match.0
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, N: SgNode<'tree>, C> Deref for SgNodeMatch<'tree, N, C> {
  type Target = N;
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
