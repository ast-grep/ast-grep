use super::strictness::MatchStrictness;
use super::Aggregator;
use crate::matcher::PatternNode;
use crate::meta_var::MetaVariable;
use crate::{Doc, Node};

pub(super) fn match_node_impl<'tree, D: Doc>(
  goal: &PatternNode,
  candidate: &Node<'tree, D>,
  agg: &mut impl Aggregator<'tree, D>,
) -> Option<()> {
  use PatternNode as P;
  match &goal {
    // leaf = without named children
    P::Terminal { text, kind_id, .. } if *kind_id == candidate.kind_id() => {
      if *text == candidate.text() {
        agg.match_terminal(candidate)
      } else {
        None
      }
    }
    P::MetaVar { meta_var, .. } => agg.match_meta_var(meta_var, candidate),
    P::Internal {
      kind_id, children, ..
    } if *kind_id == candidate.kind_id() => {
      let cand_children = candidate.children();
      match_nodes_impl_recursive(children, cand_children, agg, &MatchStrictness::Smart)
    }
    _ => None,
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
    match may_match_ellipsis_impl(&mut goal_children, &mut cand_children, agg)? {
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
    // skip if cand children is trivial
    goal_children.next();
    if goal_children.peek().is_none() {
      // all goal found, return
      return Some(());
    }
    cand_children.next();
    cand_children.peek()?;
  }
}

enum ControlFlow {
  Continue,
  Fallthrough,
  Return,
}

use std::iter::Peekable;
/// returns None means no match
fn may_match_ellipsis_impl<'p, 't: 'p, D: Doc + 't>(
  goal_children: &mut Peekable<impl Iterator<Item = &'p PatternNode>>,
  cand_children: &mut Peekable<impl Iterator<Item = Node<'t, D>>>,
  agg: &mut impl Aggregator<'t, D>,
) -> Option<ControlFlow> {
  let curr_node = goal_children.peek().unwrap();
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
    if match_node_impl(
      goal_children.peek().unwrap(),
      cand_children.peek().unwrap(),
      agg,
    )
    .is_some()
    {
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
      // if cand runs out, remaining goal is not matched
      return None;
    };
    let matched = match_node_impl(goal_children.peek().unwrap(), cand, agg).is_some();
    // try match goal node with candidate node
    if matched {
      return Some(ControlFlow::Fallthrough);
    } else if strictness.should_skip_matching_node(cand) {
      // skip trivial node
      // TODO: nade with field should not be skipped
      cand_children.next();
    } else {
      // unmatched significant node
      return None;
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
