use crate::{Doc, Node};

pub enum MatchStrictness {
  Cst,       // all nodes are matched
  Smart,     // all nodes except source trivial nodes are matched.
  Ast,       // only ast nodes are matched
  Lenient,   // ast-nodes excluding comments are matched
  Signature, // ast-nodes excluding comments, without text
}

impl MatchStrictness {
  pub fn should_skip_matching_node<D: Doc>(&self, node: &Node<D>) -> bool {
    use MatchStrictness::*;
    match self {
      Cst => false,
      Smart => !node.is_named(),
      Ast => !node.is_named(),
      Lenient => !node.is_named() || node.is_comment_like(),
      Signature => !node.is_named() || node.is_comment_like(),
    }
  }
  pub fn should_keep_in_pattern<D: Doc>(&self, node: &Node<D>) -> bool {
    use MatchStrictness::*;
    match self {
      Cst => true,
      Smart => true,
      Ast => node.is_named(),
      Lenient => node.is_named() && !node.is_comment_like(),
      Signature => node.is_named() && !node.is_comment_like(),
    }
  }
}
