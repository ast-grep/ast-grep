use crate::matcher::{MatchAll, MatchNone, Matcher};
use crate::meta_var::MetaVarEnv;
use crate::{Doc, Node};
use bit_set::BitSet;
use std::borrow::Cow;

pub struct And<P1: Matcher, P2: Matcher> {
  pattern1: P1,
  pattern2: P2,
}

impl<P1, P2> Matcher for And<P1, P2>
where
  P1: Matcher,
  P2: Matcher,
{
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    // keep the original env intact until both arms match
    let mut new_env = Cow::Borrowed(env.as_ref());
    let node = self.pattern1.match_node_with_env(node, &mut new_env)?;
    let ret = self.pattern2.match_node_with_env(node, &mut new_env)?;
    // both succeed – commit the combined env
    *env = Cow::Owned(new_env.into_owned());
    Some(ret)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    let set1 = self.pattern1.potential_kinds();
    let set2 = self.pattern2.potential_kinds();
    // if both constituent have Some(bitset), intersect them
    // otherwise returns either of the non-null set
    match (&set1, &set2) {
      (Some(s1), Some(s2)) => Some(s1.intersection(s2).collect()),
      _ => set1.xor(set2),
    }
  }
}

// we pre-compute and cache potential_kinds. So patterns should not be mutated.
// Box<[P]> is used here for immutability so that kinds will never be invalidated.
pub struct All<P: Matcher> {
  patterns: Box<[P]>,
  kinds: Option<BitSet>,
}

impl<P: Matcher> All<P> {
  pub fn new<PS: IntoIterator<Item = P>>(patterns: PS) -> Self {
    let patterns: Box<[P]> = patterns.into_iter().collect();
    let kinds = Self::compute_kinds(&patterns);
    Self { patterns, kinds }
  }

  fn compute_kinds(patterns: &[P]) -> Option<BitSet> {
    let mut set: Option<BitSet> = None;
    for pattern in patterns {
      let Some(n) = pattern.potential_kinds() else {
        continue;
      };
      if let Some(set) = set.as_mut() {
        set.intersect_with(&n);
      } else {
        set = Some(n);
      }
    }
    set
  }

  pub fn inner(&self) -> &[P] {
    &self.patterns
  }
}

impl<P: Matcher> Matcher for All<P> {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(kinds) = &self.kinds {
      if !kinds.contains(node.kind_id().into()) {
        return None;
      }
    }
    let mut new_env = Cow::Borrowed(env.as_ref());
    let all_satisfied = self
      .patterns
      .iter()
      .all(|p| p.match_node_with_env(node.clone(), &mut new_env).is_some());
    if all_satisfied {
      *env = Cow::Owned(new_env.into_owned());
      Some(node)
    } else {
      None
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.kinds.clone()
  }
}

// Box<[P]> for immutability and potential_kinds cache correctness
pub struct Any<P> {
  patterns: Box<[P]>,
  kinds: Option<BitSet>,
}

impl<P: Matcher> Any<P> {
  pub fn new<PS: IntoIterator<Item = P>>(patterns: PS) -> Self {
    let patterns: Box<[P]> = patterns.into_iter().collect();
    let kinds = Self::compute_kinds(&patterns);
    Self { patterns, kinds }
  }

  fn compute_kinds(patterns: &[P]) -> Option<BitSet> {
    let mut set = BitSet::new();
    for pattern in patterns {
      let n = pattern.potential_kinds()?;
      set.union_with(&n);
    }
    Some(set)
  }

  pub fn inner(&self) -> &[P] {
    &self.patterns
  }
}

impl<M: Matcher> Matcher for Any<M> {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(kinds) = &self.kinds {
      if !kinds.contains(node.kind_id().into()) {
        return None;
      }
    }
    let mut new_env = Cow::Borrowed(env.as_ref());
    let found = self.patterns.iter().find_map(|p| {
      new_env = Cow::Borrowed(env.as_ref());
      p.match_node_with_env(node.clone(), &mut new_env)
    });
    if found.is_some() {
      *env = Cow::Owned(new_env.into_owned());
      Some(node)
    } else {
      None
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.kinds.clone()
  }
}

pub struct Or<P1: Matcher, P2: Matcher> {
  pattern1: P1,
  pattern2: P2,
}

impl<P1, P2> Matcher for Or<P1, P2>
where
  P1: Matcher,
  P2: Matcher,
{
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let mut new_env = Cow::Borrowed(env.as_ref());
    if let Some(ret) = self
      .pattern1
      .match_node_with_env(node.clone(), &mut new_env)
    {
      *env = Cow::Owned(new_env.into_owned());
      Some(ret)
    } else {
      self.pattern2.match_node_with_env(node, env)
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    let mut set1 = self.pattern1.potential_kinds()?;
    let set2 = self.pattern2.potential_kinds()?;
    set1.union_with(&set2);
    Some(set1)
  }
}

pub struct Not<M: Matcher> {
  not: M,
}

impl<M: Matcher> Not<M> {
  pub fn new(not: M) -> Self {
    Self { not }
  }

  pub fn inner(&self) -> &M {
    &self.not
  }
}
impl<P> Matcher for Not<P>
where
  P: Matcher,
{
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    self
      .not
      .match_node_with_env(node.clone(), env)
      .xor(Some(node))
  }
}

#[derive(Clone)]
pub struct Op<M: Matcher> {
  inner: M,
}

impl<M> Matcher for Op<M>
where
  M: Matcher,
{
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    self.inner.match_node_with_env(node, env)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.inner.potential_kinds()
  }
}

/*
pub struct Predicate<F> {
  func: F,
}

impl<L, F> Matcher for Predicate<F>
where
  L: Language,
  F: for<'tree> Fn(&Node<'tree, StrDoc<L>>) -> bool,
{
  fn match_node_with_env<'tree, D: Doc<Lang=L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut MetaVarEnv<'tree, D>,
  ) -> Option<Node<'tree, D>> {
    (self.func)(&node).then_some(node)
  }
}
*/

/*
// we don't need specify M for static method
impl<L: Language> Op<L, MatchNone> {
  pub fn func<F>(func: F) -> Predicate<F>
  where
    F: for<'tree> Fn(&Node<'tree, StrDoc<L>>) -> bool,
  {
    Predicate { func }
  }
}
*/

impl<M: Matcher> Op<M> {
  pub fn not(pattern: M) -> Not<M> {
    Not { not: pattern }
  }
}

impl<M: Matcher> Op<M> {
  pub fn every(pattern: M) -> Op<And<M, MatchAll>> {
    Op {
      inner: And {
        pattern1: pattern,
        pattern2: MatchAll,
      },
    }
  }
  pub fn either(pattern: M) -> Op<Or<M, MatchNone>> {
    Op {
      inner: Or {
        pattern1: pattern,
        pattern2: MatchNone,
      },
    }
  }

  pub fn all<MS: IntoIterator<Item = M>>(patterns: MS) -> All<M> {
    All::new(patterns)
  }

  pub fn any<MS: IntoIterator<Item = M>>(patterns: MS) -> Any<M> {
    Any::new(patterns)
  }

  pub fn new(matcher: M) -> Op<M> {
    Self { inner: matcher }
  }
}

type NestedAnd<M, N, O> = And<And<M, N>, O>;
impl<M: Matcher, N: Matcher> Op<And<M, N>> {
  pub fn and<O: Matcher>(self, other: O) -> Op<NestedAnd<M, N, O>> {
    Op {
      inner: And {
        pattern1: self.inner,
        pattern2: other,
      },
    }
  }
}

type NestedOr<M, N, O> = Or<Or<M, N>, O>;
impl<M: Matcher, N: Matcher> Op<Or<M, N>> {
  pub fn or<O: Matcher>(self, other: O) -> Op<NestedOr<M, N, O>> {
    Op {
      inner: Or {
        pattern1: self.inner,
        pattern2: other,
      },
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::matcher::MatcherExt;
  use crate::meta_var::MetaVarEnv;
  use crate::Root;

  fn test_find(matcher: &impl Matcher, code: &str) {
    let node = Root::str(code, Tsx);
    assert!(matcher.find_node(node.root()).is_some());
  }
  fn test_not_find(matcher: &impl Matcher, code: &str) {
    let node = Root::str(code, Tsx);
    assert!(matcher.find_node(node.root()).is_none());
  }
  fn find_all(matcher: impl Matcher, code: &str) -> Vec<String> {
    let node = Root::str(code, Tsx);
    node
      .root()
      .find_all(matcher)
      .map(|n| n.text().to_string())
      .collect()
  }

  #[test]
  fn test_or() {
    let matcher = Or {
      pattern1: "let a = 1",
      pattern2: "const b = 2",
    };
    test_find(&matcher, "let a = 1");
    test_find(&matcher, "const b = 2");
    test_not_find(&matcher, "let a = 2");
    test_not_find(&matcher, "const a = 1");
    test_not_find(&matcher, "let b = 2");
    test_not_find(&matcher, "const b = 1");
  }

  #[test]
  fn test_not() {
    let matcher = Not { not: "let a = 1" };
    test_find(&matcher, "const b = 2");
  }

  #[test]
  fn test_and() {
    let matcher = And {
      pattern1: "let a = $_",
      pattern2: Not { not: "let a = 123" },
    };
    test_find(&matcher, "let a = 233");
    test_find(&matcher, "let a = 456");
    test_not_find(&matcher, "let a = 123");
  }

  #[test]
  fn test_api_and() {
    let matcher = Op::every("let a = $_").and(Op::not("let a = 123"));
    test_find(&matcher, "let a = 233");
    test_find(&matcher, "let a = 456");
    test_not_find(&matcher, "let a = 123");
  }

  #[test]
  fn test_api_or() {
    let matcher = Op::either("let a = 1").or("const b = 2");
    test_find(&matcher, "let a = 1");
    test_find(&matcher, "const b = 2");
    test_not_find(&matcher, "let a = 2");
    test_not_find(&matcher, "const a = 1");
    test_not_find(&matcher, "let b = 2");
    test_not_find(&matcher, "const b = 1");
  }
  #[test]
  fn test_multiple_match() {
    let sequential = find_all("$A + b", "let f = () => a + b; let ff = () => c + b");
    assert_eq!(sequential.len(), 2);
    let nested = find_all(
      "function $A() { $$$ }",
      "function a() { function b() { b } }",
    );
    assert_eq!(nested.len(), 2);
  }

  #[test]
  fn test_multiple_match_order() {
    let ret = find_all(
      "$A + b",
      "let f = () => () => () => a + b; let ff = () => c + b",
    );
    assert_eq!(ret, ["a + b", "c + b"], "should match source code order");
  }

  /*
  #[test]
  fn test_api_func() {
    let matcher = Op::func(|n| n.text().contains("114514"));
    test_find(&matcher, "let a = 114514");
    test_not_find(&matcher, "let a = 1919810");
  }
  */
  use crate::Pattern;
  trait TsxMatcher {
    fn t(self) -> Pattern;
  }
  impl TsxMatcher for &str {
    fn t(self) -> Pattern {
      Pattern::new(self, Tsx)
    }
  }

  #[test]
  fn test_and_kinds() {
    // intersect None kinds
    let matcher = Op::every("let a = $_".t()).and(Op::not("let a = 123".t()));
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    let matcher = Op::every(Op::not("let a = $_".t())).and("let a = 123".t());
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    // intersect Same kinds
    let matcher = Op::every("let a = $_".t()).and("let b = 123".t());
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    // intersect different kinds
    let matcher = Op::every("let a = 1".t()).and("console.log(1)".t());
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(0));
    // two None kinds
    let matcher = Op::every(Op::not("let a = $_".t())).and(Op::not("let a = 123".t()));
    assert_eq!(matcher.potential_kinds(), None);
  }

  #[test]
  fn test_or_kinds() {
    // union None kinds
    let matcher = Op::either("let a = $_".t()).or(Op::not("let a = 123".t()));
    assert_eq!(matcher.potential_kinds(), None);
    let matcher = Op::either(Op::not("let a = $_".t())).or("let a = 123".t());
    assert_eq!(matcher.potential_kinds(), None);
    // union Same kinds
    let matcher = Op::either("let a = $_".t()).or("let b = 123".t());
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    // union different kinds
    let matcher = Op::either("let a = 1".t()).or("console.log(1)".t());
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(2));
    // two None kinds
    let matcher = Op::either(Op::not("let a = $_".t())).or(Op::not("let a = 123".t()));
    assert_eq!(matcher.potential_kinds(), None);
  }

  #[test]
  fn test_all_kinds() {
    // intersect None kinds
    let matcher = Op::all(["let a = $_".t(), "$A".t()]);
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    let matcher = Op::all(["$A".t(), "let a = $_".t()]);
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    // intersect Same kinds
    let matcher = Op::all(["let a = $_".t(), "let b = 123".t()]);
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    // intersect different kinds
    let matcher = Op::all(["let a = 1".t(), "console.log(1)".t()]);
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(0));
    // two None kinds
    let matcher = Op::all(["$A".t(), "$B".t()]);
    assert_eq!(matcher.potential_kinds(), None);
  }

  #[test]
  fn test_any_kinds() {
    // union None kinds
    let matcher = Op::any(["let a = $_".t(), "$A".t()]);
    assert_eq!(matcher.potential_kinds(), None);
    let matcher = Op::any(["$A".t(), "let a = $_".t()]);
    assert_eq!(matcher.potential_kinds(), None);
    // union Same kinds
    let matcher = Op::any(["let a = $_".t(), "let b = 123".t()]);
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(1));
    // union different kinds
    let matcher = Op::any(["let a = 1".t(), "console.log(1)".t()]);
    assert_eq!(matcher.potential_kinds().map(|v| v.len()), Some(2));
    // two None kinds
    let matcher = Op::any(["$A".t(), "$B".t()]);
    assert_eq!(matcher.potential_kinds(), None);
  }

  #[test]
  fn test_or_revert_env() {
    let matcher = Op::either(Op::every("foo($A)".t()).and("impossible".t())).or("foo($B)".t());
    let code = Root::str("foo(123)", Tsx);
    let matches = code.root().find(matcher).expect("should found");
    assert!(matches.get_env().get_match("A").is_none());
    assert_eq!(matches.get_env().get_match("B").unwrap().text(), "123");
  }

  #[test]
  fn test_any_revert_env() {
    let matcher = Op::any([
      Op::all(["foo($A)".t(), "impossible".t()]),
      Op::all(["foo($B)".t()]),
    ]);
    let code = Root::str("foo(123)", Tsx);
    let matches = code.root().find(matcher).expect("should found");
    assert!(matches.get_env().get_match("A").is_none());
    assert_eq!(matches.get_env().get_match("B").unwrap().text(), "123");
  }

  // gh #1225
  #[test]
  fn test_all_revert_env() {
    let matcher = Op::all(["$A(123)".t(), "$B(456)".t()]);
    let code = Root::str("foo(123)", Tsx);
    let node = code.root().find("foo($C)").expect("should exist");
    let node = node.get_node().clone();
    let mut env = Cow::Owned(MetaVarEnv::new());
    assert!(matcher.match_node_with_env(node, &mut env).is_none());
    assert!(env.get_match("A").is_none());
  }
}
