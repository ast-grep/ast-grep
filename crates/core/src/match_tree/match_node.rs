use super::strictness::{MatchOneNode, MatchStrictness};
use super::Aggregator;
use crate::matcher::{kind_utils, PatternNode};
use crate::meta_var::MetaVariable;
use crate::{Doc, Node};
use std::iter::Peekable;

pub(super) fn match_node_impl<'tree, D: Doc>(
  goal: &PatternNode,
  candidate: &Node<'tree, D>,
  agg: &mut impl Aggregator<'tree, D>,
  strictness: &MatchStrictness,
) -> MatchOneNode {
  use PatternNode as P;
  match &goal {
    // leaf = without named children
    P::Terminal {
      text,
      kind_id,
      is_named,
    } => match strictness.match_terminal(*is_named, text, *kind_id, candidate) {
      MatchOneNode::MatchedBoth => {
        if agg.match_terminal(candidate).is_some() {
          MatchOneNode::MatchedBoth
        } else {
          MatchOneNode::NoMatch
        }
      }
      c => c,
    },
    P::MetaVar { meta_var, .. } => match agg.match_meta_var(meta_var, candidate) {
      Some(()) => MatchOneNode::MatchedBoth,
      None => MatchOneNode::NoMatch, // TODO: this may be wrong
    },
    P::Internal {
      kind_id, children, ..
    } if kind_utils::are_kinds_matching(*kind_id, candidate.kind_id()) => {
      let cand_children = candidate.children();
      match match_nodes_impl_recursive(children, cand_children, agg, strictness) {
        Some(()) => MatchOneNode::MatchedBoth,
        None => MatchOneNode::NoMatch,
      }
    }
    _ => MatchOneNode::NoMatch, // TODO
  }
}

fn match_nodes_impl_recursive<'tree, D: Doc + 'tree>(
  goals: &[PatternNode],
  candidates: impl Iterator<Item = Node<'tree, D>>,
  agg: &mut impl Aggregator<'tree, D>,
  strictness: &MatchStrictness,
) -> Option<()> {
  let mut goal_children = goals.iter().peekable();
  let mut cand_children = candidates.peekable();
  cand_children.peek()?;
  loop {
    match may_match_ellipsis_impl(&mut goal_children, &mut cand_children, agg, strictness)? {
      ControlFlow::Return => return Some(()),
      ControlFlow::Continue => continue,
      ControlFlow::Fallthrough => (),
    }
    match match_single_node_while_skip_trivial(
      &mut goal_children,
      &mut cand_children,
      agg,
      strictness,
    )? {
      ControlFlow::Return => return Some(()),
      ControlFlow::Continue => continue,
      ControlFlow::Fallthrough => (),
    }
    let consumed_goal = goal_children.next();
    // if goal runs out, do not proceed cand nodes
    if consumed_goal.is_some() {
      cand_children.next();
    }
    if goal_children.peek().is_none() {
      // all goal found
      let has_trailing = cand_children.all(|n| strictness.should_skip_trailing(&n));
      return has_trailing.then_some(());
    }
    cand_children.peek()?;
  }
}

enum ControlFlow {
  Continue,
  Fallthrough,
  Return,
}

/// returns None means no match
fn may_match_ellipsis_impl<'p, 't: 'p, D: Doc + 't>(
  goal_children: &mut Peekable<impl Iterator<Item = &'p PatternNode>>,
  cand_children: &mut Peekable<impl Iterator<Item = Node<'t, D>>>,
  agg: &mut impl Aggregator<'t, D>,
  strictness: &MatchStrictness,
) -> Option<ControlFlow> {
  let Some(curr_node) = goal_children.peek() else {
    // in rare case, an internal node's children is empty
    // see https://github.com/ast-grep/ast-grep/issues/1688
    return Some(ControlFlow::Return);
  };
  let Ok(optional_name) = try_get_ellipsis_mode(curr_node) else {
    return Some(ControlFlow::Fallthrough);
  };
  let mut matched = vec![];
  goal_children.next();
  // goal has all matched
  if goal_children.peek().is_none() {
    match_ellipsis(agg, &optional_name, matched, cand_children, 0)?;
    return Some(ControlFlow::Return);
  }
  // skip trivial nodes in goal after ellipsis
  let mut skipped_anonymous = 0;
  while goal_children.peek().unwrap().is_trivial() {
    goal_children.next();
    skipped_anonymous += 1;
    if goal_children.peek().is_none() {
      match_ellipsis(
        agg,
        &optional_name,
        matched,
        cand_children,
        skipped_anonymous,
      )?;
      return Some(ControlFlow::Return);
    }
  }
  // if next node is a Ellipsis, consume one candidate node
  if try_get_ellipsis_mode(goal_children.peek().unwrap()).is_ok() {
    matched.push(cand_children.next().unwrap());
    cand_children.peek()?;
    match_ellipsis(
      agg,
      &optional_name,
      matched,
      std::iter::empty(),
      skipped_anonymous,
    )?;
    return Some(ControlFlow::Continue);
  }
  loop {
    if matches!(
      match_node_impl(
        goal_children.peek().unwrap(),
        cand_children.peek().unwrap(),
        agg,
        strictness,
      ),
      MatchOneNode::MatchedBoth
    ) {
      // found match non Ellipsis,
      match_ellipsis(
        agg,
        &optional_name,
        matched,
        std::iter::empty(),
        skipped_anonymous,
      )?;
      break Some(ControlFlow::Fallthrough);
    }
    matched.push(cand_children.next().unwrap());
    cand_children.peek()?;
  }
}

fn match_single_node_while_skip_trivial<'p, 't: 'p, D: Doc + 't>(
  goal_children: &mut Peekable<impl Iterator<Item = &'p PatternNode>>,
  cand_children: &mut Peekable<impl Iterator<Item = Node<'t, D>>>,
  agg: &mut impl Aggregator<'t, D>,
  strictness: &MatchStrictness,
) -> Option<ControlFlow> {
  loop {
    let Some(cand) = cand_children.peek() else {
      // if cand runs out, check remaining goal
      // if goal is skippable, it is a match, else a non match
      return strictness
        .should_skip_goal(goal_children)
        .then_some(ControlFlow::Fallthrough);
    };
    // try match goal node with candidate node
    match match_node_impl(goal_children.peek().unwrap(), cand, agg, strictness) {
      MatchOneNode::MatchedBoth => return Some(ControlFlow::Fallthrough),
      MatchOneNode::SkipGoal => {
        goal_children.next();
        if goal_children.peek().is_none() {
          return Some(ControlFlow::Fallthrough);
        }
      }
      MatchOneNode::SkipBoth => {
        cand_children.next();
        goal_children.next();
        if goal_children.peek().is_none() {
          return Some(ControlFlow::Fallthrough);
        }
      }
      // skip trivial node
      MatchOneNode::SkipCandidate => {
        cand_children.next();
      }
      // unmatched significant node
      MatchOneNode::NoMatch => return None,
    }
  }
}

/// Returns Ok if ellipsis pattern is found. If the ellipsis is named, returns it name.
/// If the ellipsis is unnamed, returns None. If it is not ellipsis node, returns Err.
fn try_get_ellipsis_mode(node: &PatternNode) -> Result<Option<String>, ()> {
  let PatternNode::MetaVar { meta_var, .. } = node else {
    return Err(());
  };
  match meta_var {
    MetaVariable::Multiple => Ok(None),
    MetaVariable::MultiCapture(n) => Ok(Some(n.into())),
    _ => Err(()),
  }
}

fn match_ellipsis<'t, D: Doc>(
  agg: &mut impl Aggregator<'t, D>,
  optional_name: &Option<String>,
  mut matched: Vec<Node<'t, D>>,
  cand_children: impl Iterator<Item = Node<'t, D>>,
  skipped_anonymous: usize,
) -> Option<()> {
  matched.extend(cand_children);
  agg.match_ellipsis(optional_name.as_deref(), matched, skipped_anonymous)?;
  Some(())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::matcher::KindMatcher;
  use crate::{meta_var::MetaVarEnv, Matcher, Pattern, Root};
  use std::borrow::Cow;
  fn match_tree(p: &str, n: &str, strictness: MatchStrictness) -> MatchOneNode {
    let pattern = Pattern::str(p, Tsx);
    let kind = pattern.potential_kinds().expect("should have kind");
    let kind = KindMatcher::from_id(kind.into_iter().next().expect("should have kind") as u16);
    let n = Root::str(n, Tsx);
    let n = n.root().find(kind).expect("should find");
    let mut env = Cow::Owned(MetaVarEnv::new());
    match_node_impl(&pattern.node, &n, &mut env, &strictness)
  }
  fn matched(p: &str, n: &str, strictness: MatchStrictness) {
    let ret = match_tree(p, n, strictness);
    assert!(
      matches!(ret, MatchOneNode::MatchedBoth),
      "expect match. pattern: `{p}`, node: `{n}`"
    );
  }
  fn unmatched(p: &str, n: &str, strictness: MatchStrictness) {
    let ret = match_tree(p, n, strictness);
    assert!(
      !matches!(ret, MatchOneNode::MatchedBoth),
      "expect no match. pattern: `{p}`, node: `{n}`"
    );
  }
  use MatchStrictness as M;

  #[test]
  fn test_ast_match() {
    matched("import $A from 'lib'", "import A from \"lib\"", M::Ast);
    unmatched("$A(bar)", "foo(/* A*/bar)", M::Ast);
    matched("$A(bar)", "foo(bar)", M::Ast);
    unmatched("$A(bar)", "foo(bar, baz)", M::Ast);
    matched("print($A,)", "print(123)", M::Ast);
    matched("print($$$A,b,$$$C)", "print(b)", M::Ast);
    matched("print($$$A,b,$$$C)", "print(a, b)", M::Ast);
    matched("print($$$A,b,$$$C)", "print(a, b, c)", M::Ast);
    matched("print($$$A,b,$$$C)", "print(a, b, c,)", M::Ast);
  }

  #[test]
  fn test_relaxed_match() {
    matched("import $A from 'lib'", "import A from \"lib\"", M::Relaxed);
    matched("$A(bar)", "foo(/* A*/bar)", M::Relaxed);
    // fix https://github.com/ast-grep/ast-grep/issues/1848
    matched(
      "import { foo } from 'bar'",
      "import { foo, } from 'bar'",
      M::Relaxed,
    );
    unmatched(
      "import { foo } from 'bar'",
      "import { foo, bar, baz } from 'bar'",
      M::Relaxed,
    );
    unmatched(
      "import { foo } from 'bar'",
      "import { foo, bar } from 'bar'",
      M::Relaxed,
    );
  }

  #[test]
  fn test_cst_match() {
    unmatched("import $A from 'lib'", "import A from \"lib\"", M::Cst);
    unmatched("$A(bar)", "foo(/* A*/bar)", M::Cst);
    unmatched("print($A,)", "print(123)", M::Cst);
  }

  #[test]
  fn test_signature_match() {
    matched(
      "import $A from 'lib'",
      "import A from \"lib\"",
      M::Signature,
    );
    matched("$A(bar)", "foo(/* A*/bar)", M::Signature);
  }
}
