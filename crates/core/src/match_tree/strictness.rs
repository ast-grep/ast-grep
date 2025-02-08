use crate::matcher::{kind_utils, PatternNode};
use crate::meta_var::MetaVariable;
use crate::{Doc, Node};
use std::iter::Peekable;
use std::str::FromStr;

#[derive(Clone)]
pub enum MatchStrictness {
  Cst,       // all nodes are matched
  Smart,     // all nodes except source trivial nodes are matched.
  Ast,       // only ast nodes are matched
  Relaxed,   // ast-nodes excluding comments are matched
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
    goal_kind: u16,
    candidate: &Node<D>,
  ) -> MatchOneNode {
    use MatchStrictness as M;
    let cand_kind = candidate.kind_id();
    let is_kind_matched = kind_utils::are_kinds_matching(goal_kind, cand_kind);
    // work around ast-grep/ast-grep#1419 and tree-sitter/tree-sitter-typescript#306
    // tree-sitter-typescript has wrong span of unnamed node so text would not match
    // just compare kind for unnamed node
    if is_kind_matched && (!is_named || text == candidate.text()) {
      return MatchOneNode::MatchedBoth;
    }
    let (skip_goal, skip_candidate) = match self {
      M::Cst => (false, false),
      M::Smart => (false, !candidate.is_named()),
      M::Ast => (!is_named, !candidate.is_named()),
      M::Relaxed => (!is_named, skip_comment_or_unnamed(candidate)),
      M::Signature => {
        if is_kind_matched {
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

  // TODO: this is a method for working around trailing nodes after pattern is matched
  pub(crate) fn should_skip_trailing<D: Doc>(&self, candidate: &Node<D>) -> bool {
    use MatchStrictness as M;
    match self {
      M::Cst => false,
      M::Smart => true,
      M::Ast => false,
      M::Relaxed => skip_comment_or_unnamed(candidate),
      M::Signature => skip_comment_or_unnamed(candidate),
    }
  }

  pub(crate) fn should_skip_goal<'p>(
    &self,
    goal_children: &mut Peekable<impl Iterator<Item = &'p PatternNode>>,
  ) -> bool {
    use MatchStrictness as M;
    while let Some(pattern) = goal_children.peek() {
      let skipped = match self {
        M::Cst => false,
        M::Smart => match pattern {
          PatternNode::MetaVar { meta_var } => match meta_var {
            MetaVariable::Multiple => true,
            MetaVariable::MultiCapture(_) => true,
            MetaVariable::Dropped(_) => false,
            MetaVariable::Capture(..) => false,
          },
          PatternNode::Terminal { .. } => false,
          PatternNode::Internal { .. } => false,
        },
        M::Ast | M::Relaxed | M::Signature => match pattern {
          PatternNode::MetaVar { meta_var } => match meta_var {
            MetaVariable::Multiple => true,
            MetaVariable::MultiCapture(_) => true,
            MetaVariable::Dropped(named) => !named,
            MetaVariable::Capture(_, named) => !named,
          },
          PatternNode::Terminal { is_named, .. } => !is_named,
          PatternNode::Internal { .. } => false,
        },
      };
      if !skipped {
        return false;
      }
      goal_children.next();
    }
    true
  }
}

impl FromStr for MatchStrictness {
  type Err = &'static str;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "cst" => Ok(MatchStrictness::Cst),
      "smart" => Ok(MatchStrictness::Smart),
      "ast" => Ok(MatchStrictness::Ast),
      "relaxed" => Ok(MatchStrictness::Relaxed),
      "signature" => Ok(MatchStrictness::Signature),
      _ => Err("invalid strictness, valid options are: cst, smart, ast, relaxed, signature"),
    }
  }
}
