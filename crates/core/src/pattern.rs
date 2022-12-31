use crate::language::Language;
use crate::match_tree::{extract_var_from_node, match_end_non_recursive, match_node_non_recursive};
use crate::matcher::{KindMatcher, Matcher};
use crate::{meta_var::MetaVarEnv, Node, Root};

use bit_set::BitSet;

#[derive(Clone)]
pub struct Pattern<L: Language> {
  pub root: Root<L>,
  /// used in contextual pattern, specify which AST subpart is considered as pattern
  /// e.g. in js`class { $F }` we set selector to public_field_definition
  selector: Option<KindMatcher<L>>,
}

impl<L: Language> Pattern<L> {
  pub fn new(src: &str, lang: L) -> Self {
    let processed = lang.pre_process_pattern(src);
    let root = Root::new(&processed, lang);
    let goal = root.root();
    if goal.inner.child_count() != 1 {
      todo!("multi-children pattern is not supported yet.")
    }
    Self {
      root,
      selector: None,
    }
  }

  pub fn contextual(context: &str, selector: &str, lang: L) -> Self {
    let processed = lang.pre_process_pattern(context);
    let root = Root::new(&processed, lang.clone());
    let goal = root.root();
    if goal.inner.child_count() != 1 {
      todo!("multi-children pattern is not supported yet.")
    }
    let kind_matcher = KindMatcher::new(selector, lang);
    if goal.find(&kind_matcher).is_none() {
      todo!("use result to indicate failure");
    }
    Self {
      root,
      selector: Some(kind_matcher),
    }
  }

  // TODO: extract out matcher in recursion
  fn matcher(&self) -> Node<L> {
    let root = self.root.root();
    if let Some(kind_matcher) = &self.selector {
      return root
        .find(kind_matcher)
        .map(Node::from)
        .expect("contextual match should succeed");
    }
    let mut node = root.inner;
    while node.child_count() == 1 || node.child_count() == 2 && node.child(1).unwrap().is_missing()
    {
      node = node.child(0).unwrap();
    }
    Node {
      inner: node,
      root: &self.root,
    }
  }
}

impl<L: Language> Matcher<L> for Pattern<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    match_node_non_recursive(&self.matcher(), node, env)
  }

  fn potential_kinds(&self) -> Option<bit_set::BitSet> {
    if let Some(kind) = &self.selector {
      return kind.potential_kinds();
    }
    let matcher = self.matcher();
    if matcher.is_leaf() && extract_var_from_node(&matcher).is_some() {
      return None;
    }
    let mut kinds = BitSet::new();
    kinds.insert(matcher.kind_id().into());
    Some(kinds)
  }

  fn get_match_len(&self, node: Node<L>) -> Option<usize> {
    let start = node.range().start;
    let end = match_end_non_recursive(&self.matcher(), node)?;
    Some(end - start)
  }
}

impl<L: Language> std::fmt::Debug for Pattern<L> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.matcher().to_sexp())
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use std::collections::HashMap;

  fn pattern_node(s: &str) -> Root<Tsx> {
    Root::new(s, Tsx)
  }

  fn test_match(s1: &str, s2: &str) {
    let pattern = Pattern::new(s1, Tsx);
    let cand = pattern_node(s2);
    let cand = cand.root();
    assert!(
      pattern.find_node(cand.clone()).is_some(),
      "goal: {}, candidate: {}",
      pattern.root.root().to_sexp(),
      cand.to_sexp(),
    );
  }
  fn test_non_match(s1: &str, s2: &str) {
    let pattern = Pattern::new(s1, Tsx);
    let cand = pattern_node(s2);
    let cand = cand.root();
    assert!(
      pattern.find_node(cand.clone()).is_none(),
      "goal: {}, candidate: {}",
      pattern.root.root().to_sexp(),
      cand.to_sexp(),
    );
  }

  #[test]
  fn test_meta_variable() {
    test_match("const a = $VALUE", "const a = 123");
    test_match("const $VARIABLE = $VALUE", "const a = 123");
    test_match("const $VARIABLE = $VALUE", "const a = 123");
  }

  fn match_env(goal_str: &str, cand: &str) -> HashMap<String, String> {
    let pattern = Pattern::new(goal_str, Tsx);
    let cand = pattern_node(cand);
    let cand = cand.root();
    let nm = pattern.find_node(cand).unwrap();
    HashMap::from(nm.get_env().clone())
  }

  #[test]
  fn test_meta_variable_env() {
    let env = match_env("const a = $VALUE", "const a = 123");
    assert_eq!(env["VALUE"], "123");
  }

  #[test]
  fn test_match_non_atomic() {
    let env = match_env("const a = $VALUE", "const a = 5 + 3");
    assert_eq!(env["VALUE"], "5 + 3");
  }

  #[test]
  fn test_class_assignment() {
    test_match("class $C { $MEMBER = $VAL}", "class A {a = 123}");
    test_non_match("class $C { $MEMBER = $VAL; b = 123; }", "class A {a = 123}");
    // test_match("a = 123", "class A {a = 123}");
    test_non_match("a = 123", "class B {b = 123}");
  }

  #[test]
  fn test_return() {
    test_match("$A($B)", "return test(123)");
  }

  #[test]
  fn test_contextual_pattern() {
    let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
    let cand = pattern_node("class B { b = 123 }");
    assert!(pattern.find_node(cand.root()).is_some());
    let cand = pattern_node("let b = 123");
    assert!(pattern.find_node(cand.root()).is_none());
  }

  #[test]
  fn test_contextual_match_with_env() {
    let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
    let cand = pattern_node("class B { b = 123 }");
    let nm = pattern.find_node(cand.root()).expect("should match");
    let env = nm.get_env();
    let env = HashMap::from(env.clone());
    assert_eq!(env["F"], "b");
    assert_eq!(env["I"], "123");
  }

  #[test]
  fn test_contextual_unmatch_with_env() {
    let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
    let cand = pattern_node("let b = 123");
    let nm = pattern.find_node(cand.root());
    assert!(nm.is_none());
  }

  fn get_kind(kind_str: &str) -> usize {
    Tsx
      .get_ts_language()
      .id_for_node_kind(kind_str, true)
      .into()
  }

  #[test]
  fn test_pattern_potential_kinds() {
    let pattern = Pattern::new("const a = 1", Tsx);
    let kind = get_kind("lexical_declaration");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_pattern_with_non_root_meta_var() {
    let pattern = Pattern::new("const $A = $B", Tsx);
    let kind = get_kind("lexical_declaration");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_bare_wildcard() {
    let pattern = Pattern::new("$A", Tsx);
    // wildcard should match anything, so kinds should be None
    assert!(pattern.potential_kinds().is_none());
  }

  #[test]
  fn test_contextual_potential_kinds() {
    let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
    let kind = get_kind("public_field_definition");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_contextual_wildcard() {
    let pattern = Pattern::contextual("class A { $F }", "property_identifier", Tsx);
    let kind = get_kind("property_identifier");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }
}
