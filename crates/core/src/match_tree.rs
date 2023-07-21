use crate::meta_var::{MetaVarEnv, MetaVariable};
use crate::{Doc, Language, Node};

use std::borrow::Cow;

fn match_leaf_meta_var<'tree, D: Doc>(
  goal: &Node<impl Doc>,
  candidate: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  let extracted = extract_var_from_node(goal)?;
  use MetaVariable as MV;
  match extracted {
    MV::Named(name, named) => {
      if named && !candidate.is_named() {
        None
      } else {
        env.to_mut().insert(name, candidate.clone())?;
        Some(candidate)
      }
    }
    MV::Anonymous(named) => {
      if named && !candidate.is_named() {
        None
      } else {
        Some(candidate)
      }
    }
    // Ellipsis will be matched in parent level
    MV::Ellipsis => {
      debug_assert!(false, "Ellipsis should be matched in parent level");
      Some(candidate)
    }
    MV::NamedEllipsis(name) => {
      env.to_mut().insert(name, candidate.clone())?;
      Some(candidate)
    }
  }
}

#[inline]
fn is_node_eligible_for_meta_var(goal: &Node<impl Doc>, is_leaf: bool) -> bool {
  // allow Error as meta_var
  // see https://github.com/ast-grep/ast-grep/issues/526
  is_leaf || goal.is_error()
}

fn try_get_ellipsis_mode(node: &Node<impl Doc>) -> Result<Option<String>, ()> {
  match extract_var_from_node(node).ok_or(())? {
    MetaVariable::Ellipsis => Ok(None),
    MetaVariable::NamedEllipsis(n) => Ok(Some(n)),
    _ => Err(()),
  }
}

fn update_ellipsis_env<'t, D: Doc>(
  optional_name: &Option<String>,
  mut matched: Vec<Node<'t, D>>,
  env: &mut Cow<MetaVarEnv<'t, D>>,
  cand_children: impl Iterator<Item = Node<'t, D>>,
  skipped_anonymous: usize,
) -> Option<()> {
  if let Some(name) = optional_name.as_ref() {
    matched.extend(cand_children);
    let skipped = matched.len().saturating_sub(skipped_anonymous);
    drop(matched.drain(skipped..));
    env.to_mut().insert_multi(name.to_string(), matched)?;
  }
  Some(())
}

pub fn match_end_non_recursive<D: Doc>(
  goal: &Node<impl Doc<Lang = D::Lang>>,
  candidate: Node<D>,
) -> Option<usize> {
  let is_leaf = goal.is_named_leaf();
  if is_node_eligible_for_meta_var(goal, is_leaf) && extract_var_from_node(goal).is_some() {
    return Some(candidate.range().end);
  }
  if goal.kind_id() != candidate.kind_id() {
    return None;
  }
  if is_leaf {
    if extract_var_from_node(goal).is_some() {
      return None;
    }
    return if goal.text() == candidate.text() {
      Some(candidate.range().end)
    } else {
      None
    };
  }
  let goal_children = goal.children();
  let cand_children = candidate.children();
  match_multi_nodes_end_non_recursive(goal_children, cand_children)
}

fn match_multi_nodes_end_non_recursive<'g, 'c, D: Doc + 'c>(
  goals: impl Iterator<Item = Node<'g, impl Doc<Lang = D::Lang> + 'g>>,
  candidates: impl Iterator<Item = Node<'c, D>>,
) -> Option<usize> {
  let mut goal_children = goals.peekable();
  let mut cand_children = candidates.peekable();
  let mut end = cand_children.peek()?.range().end;
  loop {
    let curr_node = goal_children.peek().unwrap();
    if try_get_ellipsis_mode(curr_node).is_ok() {
      goal_children.next();
      // goal has all matched
      if goal_children.peek().is_none() {
        // TODO: handle named and unnamed ellipsis
        // we need to consume all cand_children to match ellipsis
        let updated_end = cand_children.last().map(|n| n.range().end).unwrap_or(end);
        return Some(updated_end);
      }
      // skip trivial nodes in goal after ellipsis
      while !goal_children.peek().unwrap().is_named() {
        goal_children.next();
        if goal_children.peek().is_none() {
          // TODO: handle named and unnamed ellipsis
          // we need to consume all cand_children to match ellipsis
          let updated_end = cand_children.last().map(|n| n.range().end).unwrap_or(end);
          return Some(updated_end);
        }
      }
      // if next node is a Ellipsis, consume one candidate node
      if try_get_ellipsis_mode(goal_children.peek().unwrap()).is_ok() {
        cand_children.next();
        cand_children.peek()?;
        continue;
      }
      loop {
        if match_end_non_recursive(
          goal_children.peek().unwrap(),
          cand_children.peek().unwrap().clone(),
        )
        .is_some()
        {
          // found match non Ellipsis,
          break;
        }
        cand_children.next();
        cand_children.peek()?;
      }
    }
    // skip if cand children is trivial
    end = loop {
      let Some(cand) = cand_children.peek() else {
        // if cand runs out, remaining goal is not matched
        return None;
      };
      let matched_end = match_end_non_recursive(goal_children.peek().unwrap(), cand.clone());
      // try match goal node with candidate node
      if let Some(end) = matched_end {
        break end;
      } else if !cand.is_named() {
        // skip trivial node
        // TODO: nade with field should not be skipped
        cand_children.next();
      } else {
        // unmatched significant node
        return None;
      }
    };
    goal_children.next();
    if goal_children.peek().is_none() {
      // all goal found, return
      return Some(end);
    }
    cand_children.next();
    cand_children.peek()?;
  }
}

pub fn match_node_non_recursive<'tree, D: Doc>(
  goal: &Node<impl Doc>,
  candidate: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  let is_leaf = goal.is_named_leaf();
  if is_node_eligible_for_meta_var(goal, is_leaf) {
    if let Some(matched) = match_leaf_meta_var(goal, candidate.clone(), env) {
      return Some(matched);
    }
  }
  if goal.kind_id() != candidate.kind_id() {
    return None;
  }
  if is_leaf {
    if extract_var_from_node(goal).is_some() {
      return None;
    }
    return if goal.text() == candidate.text() {
      Some(candidate)
    } else {
      None
    };
  }
  let goal_children = goal.children();
  let cand_children = candidate.children();
  if match_nodes_non_recursive(goal_children, cand_children, env).is_some() {
    Some(candidate)
  } else {
    None
  }
}

fn match_nodes_non_recursive<'goal, 'tree, D: Doc + 'tree>(
  goals: impl Iterator<Item = Node<'goal, impl Doc + 'goal>>,
  candidates: impl Iterator<Item = Node<'tree, D>>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<()> {
  let mut goal_children = goals.peekable();
  let mut cand_children = candidates.peekable();
  cand_children.peek()?;
  loop {
    let curr_node = goal_children.peek().unwrap();
    if let Ok(optional_name) = try_get_ellipsis_mode(curr_node) {
      let mut matched = vec![];
      goal_children.next();
      // goal has all matched
      if goal_children.peek().is_none() {
        update_ellipsis_env(&optional_name, matched, env, cand_children, 0)?;
        return Some(());
      }
      // skip trivial nodes in goal after ellipsis
      let mut skipped_anonymous = 0;
      while !goal_children.peek().unwrap().is_named() {
        goal_children.next();
        skipped_anonymous += 1;
        if goal_children.peek().is_none() {
          update_ellipsis_env(
            &optional_name,
            matched,
            env,
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
        update_ellipsis_env(
          &optional_name,
          matched,
          env,
          std::iter::empty(),
          skipped_anonymous,
        )?;
        continue;
      }
      loop {
        if match_node_non_recursive(
          goal_children.peek().unwrap(),
          cand_children.peek().unwrap().clone(),
          env,
        )
        .is_some()
        {
          // found match non Ellipsis,
          update_ellipsis_env(
            &optional_name,
            matched,
            env,
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
      let matched =
        match_node_non_recursive(goal_children.peek().unwrap(), cand.clone(), env).is_some();
      // try match goal node with candidate node
      if matched {
        break;
      } else if !cand.is_named() {
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

pub fn does_node_match_exactly<D: Doc>(goal: &Node<D>, candidate: &Node<D>) -> bool {
  if goal.kind_id() != candidate.kind_id() {
    return false;
  }
  if goal.is_named_leaf() {
    return goal.text() == candidate.text();
  }
  let goal_children = goal.children();
  let cand_children = candidate.children();
  if goal_children.len() != cand_children.len() {
    return false;
  }
  goal_children
    .zip(cand_children)
    .all(|(g, c)| does_node_match_exactly(&g, &c))
}

pub fn extract_var_from_node<D: Doc>(goal: &Node<D>) -> Option<MetaVariable> {
  let key = goal.text();
  goal.lang().extract_meta_var(&key)
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::{Root, StrDoc};
  use std::collections::HashMap;

  fn find_node_recursive<'tree>(
    goal: &Node<StrDoc<Tsx>>,
    node: Node<'tree, StrDoc<Tsx>>,
    env: &mut Cow<MetaVarEnv<'tree, StrDoc<Tsx>>>,
  ) -> Option<Node<'tree, StrDoc<Tsx>>> {
    match_node_non_recursive(goal, node.clone(), env).or_else(|| {
      node
        .children()
        .find_map(|sub| find_node_recursive(goal, sub, env))
    })
  }

  fn test_match(s1: &str, s2: &str) -> HashMap<String, String> {
    let goal = Root::new(s1, Tsx);
    let goal = goal.root().child(0).unwrap();
    let cand = Root::new(s2, Tsx);
    let cand = cand.root();
    let mut env = Cow::Owned(MetaVarEnv::new());
    let ret = find_node_recursive(&goal, cand.clone(), &mut env);
    assert!(
      ret.is_some(),
      "goal: {}, candidate: {}",
      goal.to_sexp(),
      cand.to_sexp(),
    );
    HashMap::from(env.into_owned())
  }

  fn test_non_match(s1: &str, s2: &str) {
    let goal = Root::new(s1, Tsx);
    let goal = goal.root().child(0).unwrap();
    let cand = Root::new(s2, Tsx);
    let cand = cand.root();
    let mut env = Cow::Owned(MetaVarEnv::new());
    let ret = find_node_recursive(&goal, cand, &mut env);
    assert!(ret.is_none());
  }

  #[test]
  fn test_simple_match() {
    test_match("const a = 123", "const a=123");
    test_non_match("const a = 123", "var a = 123");
  }

  #[test]
  fn test_nested_match() {
    test_match("const a = 123", "function() {const a= 123;}");
    test_match("const a = 123", "class A { constructor() {const a= 123;}}");
    test_match(
      "const a = 123",
      "for (let a of []) while (true) { const a = 123;}",
    );
  }

  #[test]
  fn test_should_exactly_match() {
    test_match(
      "function foo() { let a = 123; }",
      "function foo() { let a = 123; }",
    );
    test_non_match(
      "function foo() { let a = 123; }",
      "function bar() { let a = 123; }",
    );
  }

  #[test]
  fn test_match_inner() {
    test_match(
      "function bar() { let a = 123; }",
      "function foo() { function bar() {let a = 123; }}",
    );
    test_non_match(
      "function foo() { let a = 123; }",
      "function foo() { function bar() {let a = 123; }}",
    );
  }

  #[test]
  fn test_single_ellipsis() {
    test_match("foo($$$)", "foo(a, b, c)");
    test_match("foo($$$)", "foo()");
  }
  #[test]
  fn test_named_ellipsis() {
    test_match("foo($$$A, c)", "foo(a, b, c)");
    test_match("foo($$$A, b, c)", "foo(a, b, c)");
    test_match("foo($$$A, a, b, c)", "foo(a, b, c)");
    test_non_match("foo($$$A, a, b, c)", "foo(b, c)");
  }

  #[test]
  fn test_leading_ellipsis() {
    test_match("foo($$$, c)", "foo(a, b, c)");
    test_match("foo($$$, b, c)", "foo(a, b, c)");
    test_match("foo($$$, a, b, c)", "foo(a, b, c)");
    test_non_match("foo($$$, a, b, c)", "foo(b, c)");
  }
  #[test]
  fn test_trailing_ellipsis() {
    test_match("foo(a, $$$)", "foo(a, b, c)");
    test_match("foo(a, b, $$$)", "foo(a, b, c)");
    // test_match("foo(a, b, c, $$$)", "foo(a, b, c)");
    test_non_match("foo(a, b, c, $$$)", "foo(b, c)");
  }

  #[test]
  fn test_meta_var_named() {
    test_match("return $A", "return 123;");
    test_match("return $_", "return 123;");
    test_non_match("return $A", "return;");
    test_non_match("return $_", "return;");
    test_match("return $$A", "return;");
    test_match("return $$_A", "return;");
  }

  #[test]
  fn test_meta_var_multiple_occurrence() {
    test_match("$A($$$)", "test(123)");
    test_match("$A($B)", "test(123)");
    test_non_match("$A($A)", "test(aaa)");
    test_non_match("$A($A)", "test(123)");
    test_non_match("$A($A, $A)", "test(123, 456)");
    test_match("$A($A)", "test(test)");
  }

  #[test]
  fn test_string() {
    test_match("'a'", "'a'");
    test_match("'abcdefg'", "'abcdefg'");
    test_match("`abcdefg`", "`abcdefg`");
    test_non_match("'a'", "'b'");
    test_non_match("'abcdefg'", "'gggggg'");
  }

  #[test]
  fn test_skip_trivial_node() {
    test_match("foo($A, $B)", "foo(a, b,)");
    test_match("class A { b() {}}", "class A { get b() {}}");
  }

  #[test]
  fn test_trivias_in_pattern() {
    test_match("foo($A, $B,)", "foo(a, b,)");
    test_non_match("foo($A, $B,)", "foo(a, b)");
    test_match("class A { get b() {}}", "class A { get b() {}}");
    test_non_match("class A { get b() {}}", "class A { b() {}}");
  }

  fn find_end_recursive(goal: &Node<StrDoc<Tsx>>, node: Node<StrDoc<Tsx>>) -> Option<usize> {
    match_end_non_recursive(goal, node.clone()).or_else(|| {
      node
        .children()
        .find_map(|sub| find_end_recursive(goal, sub))
    })
  }

  fn test_end(s1: &str, s2: &str) -> Option<usize> {
    let goal = Root::new(s1, Tsx);
    let goal = goal.root().child(0).unwrap();
    let cand = Root::new(s2, Tsx);
    let cand = cand.root();
    find_end_recursive(&goal, cand.clone())
  }

  #[test]
  fn test_match_end() {
    let end = test_end("return $A", "return 123 /* trivia */");
    assert_eq!(end.expect("should work"), 10);
    let end = test_end("return f($A)", "return f(1,) /* trivia */");
    assert_eq!(end.expect("should work"), 12);
  }

  // see https://github.com/ast-grep/ast-grep/issues/411
  #[test]
  fn test_ellipsis_end() {
    let end = test_end(
      "import {$$$A, B, $$$C} from 'a'",
      "import {A, B, C} from 'a'",
    );
    assert_eq!(end.expect("must match"), 25);
  }
}
