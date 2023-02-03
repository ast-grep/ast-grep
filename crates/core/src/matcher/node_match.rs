use crate::meta_var::MetaVarEnv;
use crate::replacer::Replacer;
use crate::ts_parser::Edit;
use crate::Language;
use crate::Node;

use std::borrow::Borrow;
use std::ops::Deref;

/// Represents the matched node with populated MetaVarEnv.
/// It derefs to the Node so you can use it as a Node.
/// To access the underlying MetaVarEnv, call `get_env` method.
#[derive(Clone)]
pub struct NodeMatch<'tree, L: Language>(Node<'tree, L>, MetaVarEnv<'tree, L>);

impl<'tree, L: Language> NodeMatch<'tree, L> {
  pub fn new(node: Node<'tree, L>, env: MetaVarEnv<'tree, L>) -> Self {
    Self(node, env)
  }

  pub fn get_node(&self) -> &Node<'tree, L> {
    &self.0
  }

  /// Returns the populated MetaVarEnv for this match.
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
  /// # Safety
  /// should only called for readopting nodes
  pub(crate) unsafe fn get_mut_node(&mut self) -> &mut Node<'tree, L> {
    &mut self.0
  }
}

impl<'tree, L: Language> From<Node<'tree, L>> for NodeMatch<'tree, L> {
  fn from(node: Node<'tree, L>) -> Self {
    Self(node, MetaVarEnv::new())
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, L: Language> From<NodeMatch<'tree, L>> for Node<'tree, L> {
  fn from(node_match: NodeMatch<'tree, L>) -> Self {
    node_match.0
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, L: Language> Deref for NodeMatch<'tree, L> {
  type Target = Node<'tree, L>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// NodeMatch is an immutable view to Node
impl<'tree, L: Language> Borrow<Node<'tree, L>> for NodeMatch<'tree, L> {
  fn borrow(&self) -> &Node<'tree, L> {
    &self.0
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;

  fn use_node<L: Language>(n: &Node<L>) -> String {
    n.text().to_string()
  }

  fn borrow_node<'a, L, B>(b: B) -> String
  where
    L: Language + 'static,
    B: Borrow<Node<'a, L>>,
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
    assert_eq!(fixed.inserted_text, "var b = a");
  }
}
