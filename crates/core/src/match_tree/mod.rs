mod match_node;
mod strictness;

use match_node::match_node_impl;
use strictness::MatchOneNode;
pub use strictness::MatchStrictness;

use crate::meta_var::{MetaVarEnv, MetaVariable};
use crate::{Doc, Node, Pattern};

use std::borrow::Cow;

trait Aggregator<'t, D: Doc> {
  fn match_terminal(&mut self, node: &Node<'t, D>) -> Option<()>;
  fn match_meta_var(&mut self, var: &MetaVariable, node: &Node<'t, D>) -> Option<()>;
  fn match_ellipsis(
    &mut self,
    var: Option<&str>,
    nodes: Vec<Node<'t, D>>,
    skipped_anonymous: usize,
  ) -> Option<()>;
}

struct ComputeEnd(usize);

impl<'t, D: Doc> Aggregator<'t, D> for ComputeEnd {
  fn match_terminal(&mut self, node: &Node<'t, D>) -> Option<()> {
    self.0 = node.range().end;
    Some(())
  }
  fn match_meta_var(&mut self, _: &MetaVariable, node: &Node<'t, D>) -> Option<()> {
    self.0 = node.range().end;
    Some(())
  }
  fn match_ellipsis(
    &mut self,
    _var: Option<&str>,
    nodes: Vec<Node<'t, D>>,
    _skipped: usize,
  ) -> Option<()> {
    let n = nodes.last()?;
    self.0 = n.range().end;
    Some(())
  }
}

pub fn match_end_non_recursive<D: Doc>(
  goal: &Pattern<D::Lang>,
  candidate: Node<D>,
) -> Option<usize> {
  let mut end = ComputeEnd(0);
  match match_node_impl(&goal.node, &candidate, &mut end, &goal.strictness) {
    MatchOneNode::MatchedBoth => Some(end.0),
    _ => None,
  }
}

fn match_leaf_meta_var<'tree, D: Doc>(
  mv: &MetaVariable,
  candidate: &Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<()> {
  use MetaVariable as MV;
  match mv {
    MV::Capture(name, named) => {
      if *named && !candidate.is_named() {
        None
      } else {
        env.to_mut().insert(name, candidate.clone())?;
        Some(())
      }
    }
    MV::Dropped(named) => {
      if *named && !candidate.is_named() {
        None
      } else {
        Some(())
      }
    }
    // Ellipsis will be matched in parent level
    MV::Multiple => {
      debug_assert!(false, "Ellipsis should be matched in parent level");
      Some(())
    }
    MV::MultiCapture(name) => {
      env.to_mut().insert(name, candidate.clone())?;
      Some(())
    }
  }
}

impl<'t, D: Doc> Aggregator<'t, D> for Cow<'_, MetaVarEnv<'t, D>> {
  fn match_terminal(&mut self, _: &Node<'t, D>) -> Option<()> {
    Some(())
  }
  fn match_meta_var(&mut self, var: &MetaVariable, node: &Node<'t, D>) -> Option<()> {
    match_leaf_meta_var(var, node, self)
  }
  fn match_ellipsis(
    &mut self,
    var: Option<&str>,
    nodes: Vec<Node<'t, D>>,
    skipped_anonymous: usize,
  ) -> Option<()> {
    if let Some(var) = var {
      let mut matched = nodes;
      let skipped = matched.len().saturating_sub(skipped_anonymous);
      drop(matched.drain(skipped..));
      self.to_mut().insert_multi(var, matched)?;
    }
    Some(())
  }
}

pub fn match_node_non_recursive<'tree, D: Doc>(
  goal: &Pattern<D::Lang>,
  candidate: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  match match_node_impl(&goal.node, &candidate, env, &goal.strictness) {
    MatchOneNode::MatchedBoth => Some(candidate),
    _ => None,
  }
}

pub fn does_node_match_exactly<D: Doc>(goal: &Node<D>, candidate: &Node<D>) -> bool {
  // return true if goal and candidate are the same node
  if goal.node_id() == candidate.node_id() {
    return true;
  }
  // gh issue #1087, we make pattern matching a little bit more permissive
  // compare node text if at least one node is leaf
  if goal.is_named_leaf() || candidate.is_named_leaf() {
    return goal.text() == candidate.text();
  }
  if goal.kind_id() != candidate.kind_id() {
    return false;
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

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::{Root, StrDoc};
  use std::collections::HashMap;

  fn find_node_recursive<'tree>(
    goal: &Pattern<Tsx>,
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
    let goal = Pattern::new(s1, Tsx);
    let cand = Root::new(s2, Tsx);
    let cand = cand.root();
    let mut env = Cow::Owned(MetaVarEnv::new());
    let ret = find_node_recursive(&goal, cand.clone(), &mut env);
    assert!(
      ret.is_some(),
      "goal: {goal:?}, candidate: {}",
      cand.to_sexp(),
    );
    HashMap::from(env.into_owned())
  }

  fn test_non_match(s1: &str, s2: &str) {
    let goal = Pattern::new(s1, Tsx);
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
    test_non_match("$A($A)", "foo(bar)");
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
  fn test_trivia_in_pattern() {
    test_match("foo($A, $B,)", "foo(a, b,)");
    test_non_match("foo($A, $B,)", "foo(a, b)");
    test_match("class A { get b() {}}", "class A { get b() {}}");
    test_non_match("class A { get b() {}}", "class A { b() {}}");
  }

  fn find_end_recursive(goal: &Pattern<Tsx>, node: Node<StrDoc<Tsx>>) -> Option<usize> {
    match_end_non_recursive(goal, node.clone()).or_else(|| {
      node
        .children()
        .find_map(|sub| find_end_recursive(goal, sub))
    })
  }

  fn test_end(s1: &str, s2: &str) -> Option<usize> {
    let goal = Pattern::new(s1, Tsx);
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

  #[test]
  fn test_gh_1087() {
    test_match("($P) => $F($P)", "(x) => bar(x)");
  }
}
