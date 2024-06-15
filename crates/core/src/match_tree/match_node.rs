use super::strictness::MatchStrictness;
use super::Aggregator;
use crate::meta_var::MetaVariable;
use crate::{Doc, Language, Node, Pattern};

pub(super) fn match_node_impl<'tree, D: Doc>(
  goal: &Pattern<D::Lang>,
  candidate: &Node<'tree, D>,
  agg: &mut impl Aggregator<'tree, D>,
) -> Option<()> {
  use Pattern as P;
  match goal {
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
  goals: &[Pattern<D::Lang>],
  candidates: impl Iterator<Item = Node<'tree, D>>,
  agg: &mut impl Aggregator<'tree, D>,
  strictness: &MatchStrictness,
) -> Option<()> {
  let mut goal_children = goals.iter().peekable();
  let mut cand_children = candidates.peekable();
  cand_children.peek()?;
  loop {
    let curr_node = goal_children.peek().unwrap();
    if let Ok(optional_name) = try_get_ellipsis_mode(curr_node) {
      let mut matched = vec![];
      goal_children.next();
      // goal has all matched
      if goal_children.peek().is_none() {
        match_ellipsis(agg, &optional_name, matched, cand_children, 0)?;
        return Some(());
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
          return Some(());
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
        continue;
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
          break;
        }
        matched.push(cand_children.next().unwrap());
        cand_children.peek()?;
      }
    }
    // skip if cand children is trivial
    loop {
      let Some(cand) = cand_children.peek() else {
        // if cand runs out, remaining goal is not matched
        return None;
      };
      let matched = match_node_impl(goal_children.peek().unwrap(), cand, agg).is_some();
      // try match goal node with candidate node
      if matched {
        break;
      } else if strictness.should_skip_matching_node(cand) {
        // skip trivial node
        // TODO: nade with field should not be skipped
        cand_children.next();
      } else {
        // unmatched significant node
        return None;
      }
    }
    goal_children.next();
    if goal_children.peek().is_none() {
      // all goal found, return
      return Some(());
    }
    cand_children.next();
    cand_children.peek()?;
  }
}

/// Returns Ok if ellipsis pattern is found. If the ellipsis is named, returns it name.
/// If the ellipsis is unnamed, returns None. If it is not ellipsis node, returns Err.
fn try_get_ellipsis_mode(node: &Pattern<impl Language>) -> Result<Option<String>, ()> {
  let Pattern::MetaVar { meta_var, .. } = node else {
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
