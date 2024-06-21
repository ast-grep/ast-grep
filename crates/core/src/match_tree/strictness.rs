use crate::{Doc, Node};

#[derive(Clone)]
pub enum MatchStrictness {
  Cst,       // all nodes are matched
  Smart,     // all nodes except source trivial nodes are matched.
  Ast,       // only ast nodes are matched
  Lenient,   // ast-nodes excluding comments are matched
  Signature, // ast-nodes excluding comments, without text
}

pub(crate) enum MatchOneNode {
  MatchedBoth,
  SkipBoth,
  SkipGoal,
  SkipCandidate,
  NoMatch,
}

fn skip_comment_or_unnamed(n: &Node<impl Doc>) -> bool {
  if !n.is_named() {
    return true;
  }
  let kind = n.kind();
  kind.contains("comment")
}

impl MatchStrictness {
  pub(crate) fn match_terminal<D: Doc>(
    &self,
    is_named: bool,
    text: &str,
    kind: u16,
    candidate: &Node<D>,
  ) -> MatchOneNode {
    use MatchStrictness as M;
    let k = candidate.kind_id();
    if k == kind && text == candidate.text() {
      return MatchOneNode::MatchedBoth;
    }
    let (skip_goal, skip_candidate) = match self {
      M::Cst => (false, false),
      M::Smart => (false, !candidate.is_named()),
      M::Ast => (!is_named, !candidate.is_named()),
      M::Lenient => (!is_named, skip_comment_or_unnamed(candidate)),
      M::Signature => {
        if k == kind {
          return MatchOneNode::MatchedBoth;
        }
        (!is_named, skip_comment_or_unnamed(candidate))
      }
    };
    match (skip_goal, skip_candidate) {
      (true, true) => MatchOneNode::SkipBoth,
      (true, false) => MatchOneNode::SkipGoal,
      (false, true) => MatchOneNode::SkipCandidate,
      (false, false) => MatchOneNode::NoMatch,
    }
  }
}
