use crate::{Doc, Node};

pub enum MatchStrictness {
  Cst,         // all nodes are matched
  Smart,       // all nodes except source trivial nodes are matched.
  Significant, // only significant nodes are matched
  Ast,         // only ast nodes are matched
  Lenient,     // ast-nodes excluding comments are matched
}

impl MatchStrictness {
  pub fn should_skip_matching_node<D: Doc>(&self, node: &Node<D>) -> bool {
    use MatchStrictness::*;
    match self {
      Cst => todo!(),
      Smart => !node.is_named(),
      Significant => todo!(),
      Ast => todo!(),
      Lenient => todo!(),
    }
  }
  pub fn should_keep_in_pattern<D: Doc>(&self, node: &Node<D>) -> bool {
    use MatchStrictness::*;
    match self {
      Cst => true,
      Smart => true,
      Significant => todo!("named + has field"),
      Ast => node.is_named(),
      Lenient => todo!("skip comment like"),
    }
  }
}
