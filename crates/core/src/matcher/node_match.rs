use crate::meta_var::MetaVarEnv;
use crate::replacer::Replacer;
use crate::ts_parser::Edit;
use crate::Language;
use crate::Node;

use std::borrow::{Borrow, BorrowMut};
use std::ops::{Deref, DerefMut};

/// Represents the matched node with populated MetaVarEnv.
/// It derefs to the Node so you can use it as a Node.
/// To access the underlying MetaVarEnv, call `get_env` method.
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
  // TODO: add NodeMatch test
}
