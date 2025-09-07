use crate::matcher::{kind_utils, PatternNode};
use crate::meta_var::MetaVariable;
use crate::node::Node;
use crate::Doc;
use std::iter::Peekable;
use std::str::FromStr;

#[derive(Clone)]
pub enum MatchStrictness {
  Cst,       // all nodes are matched
  Smart,     // all nodes except source trivial nodes are matched.
  Ast,       // only ast nodes are matched
  Relaxed,   // ast-nodes excluding comments are matched
  Signature, // ast-nodes excluding comments, without text
  Template,  // similar to smart, but node kinds are ignored, only text is matched.
}

pub(crate) enum MatchOneNode {
  MatchedBoth,
  SkipBoth,
  SkipGoal,
  SkipCandidate,
  NoMatch,
}

fn skip_comment(n: &Node<impl Doc>) -> bool {
  n.kind().contains("comment")
}

fn skip_comment_or_unnamed(n: &Node<impl Doc>) -> bool {
  if !n.is_named() {
    return true;
  }
  skip_comment(n)
}

impl MatchStrictness {
  pub(crate) fn should_skip_kind(&self) -> bool {
    use MatchStrictness as M;
    match self {
      M::Template => true,
      M::Cst => false,
      M::Smart => false,
      M::Ast => false,
      M::Relaxed => false,
      M::Signature => false,
    }
  }

  fn should_skip_comment(&self) -> bool {
    use MatchStrictness as M;
    match self {
      M::Cst | M::Smart | M::Ast => false,
      M::Relaxed | M::Signature | M::Template => true,
    }
  }

  pub(crate) fn match_terminal(
    &self,
    is_named: bool,
    text: &str,
    goal_kind: u16,
    candidate: &Node<impl Doc>,
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
    if self.should_skip_comment() && skip_comment(candidate) {
      return MatchOneNode::SkipCandidate;
    }
    let (skip_goal, skip_candidate) = match self {
      M::Cst => (false, false),
      M::Smart => (false, !candidate.is_named()),
      M::Ast => (!is_named, !candidate.is_named()),
      M::Relaxed => (!is_named, !candidate.is_named()),
      M::Signature => {
        if is_kind_matched {
          return MatchOneNode::MatchedBoth;
        }
        (!is_named, !candidate.is_named())
      }
      M::Template => {
        if text == candidate.text() {
          return MatchOneNode::MatchedBoth;
        } else {
          (false, !candidate.is_named())
        }
      }
    };
    match (skip_goal, skip_candidate) {
      (true, true) => MatchOneNode::SkipBoth,
      (true, false) => MatchOneNode::SkipGoal,
      (false, true) => MatchOneNode::SkipCandidate,
      (false, false) => MatchOneNode::NoMatch,
    }
  }

  pub(crate) fn should_skip_cand_for_metavar<D: Doc>(&self, candidate: &Node<D>) -> bool {
    use MatchStrictness as M;
    match self {
      M::Cst | M::Ast | M::Smart => false,
      M::Relaxed | M::Signature | M::Template => skip_comment(candidate),
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
      M::Template => skip_comment(candidate),
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
        M::Smart | M::Template => match pattern {
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
      "template" => Ok(MatchStrictness::Template),
      _ => Err("invalid strictness, valid options are: cst, smart, ast, relaxed, signature"),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::{Pattern, Root};

  fn test_match(p: &str, n: &str, strictness: MatchStrictness) -> bool {
    let mut pattern = Pattern::new(p, Tsx);
    pattern.strictness = strictness;
    let root = Root::str(n, Tsx);
    let node = root.root();
    node.find(pattern).is_some()
  }

  fn template_pattern(p: &str, n: &str) -> bool {
    test_match(p, n, MatchStrictness::Template)
  }

  #[test]
  fn test_template_pattern() {
    assert!(template_pattern("$A = $B", "a = b"));
    assert!(template_pattern("$A = $B", "var a = b"));
    assert!(template_pattern("$A = $B", "let a = b"));
    assert!(template_pattern("$A = $B", "const a = b"));
    assert!(template_pattern("$A = $B", "class A { a = b }"));
  }

  fn relaxed_pattern(p: &str, n: &str) -> bool {
    test_match(p, n, MatchStrictness::Relaxed)
  }

  #[test]
  fn test_ignore_comment() {
    assert!(relaxed_pattern("$A($B)", "foo(bar /* .. */)"));
    assert!(relaxed_pattern(
      "$A($B)",
      "
      foo(
        bar, // ..
      )"
    ));
    assert!(relaxed_pattern("$A($B)", "foo(/* .. */ bar)"));
    assert!(relaxed_pattern(
      "$A($B)",
      "
      foo( // ..
        bar
      )"
    ));
  }
}
