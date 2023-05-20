use crate::matcher::{MatchAll, MatchNone, Matcher};
use crate::meta_var::MetaVarEnv;
// use crate::meta_var::{MetaVarMatcher, MetaVarMatchers};
use crate::{Doc, Language, Node};
use bit_set::BitSet;
use std::borrow::Cow;
use std::marker::PhantomData;

pub struct And<L: Language, P1: Matcher<L>, P2: Matcher<L>> {
  pattern1: P1,
  pattern2: P2,
  lang: PhantomData<L>,
}

impl<L: Language, P1, P2> Matcher<L> for And<L, P1, P2>
where
  P1: Matcher<L>,
  P2: Matcher<L>,
{
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let node = self.pattern1.match_node_with_env(node, env)?;
    self.pattern2.match_node_with_env(node, env)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    let set1 = self.pattern1.potential_kinds();
    let set2 = self.pattern2.potential_kinds();
    // if both constituent have Some(bitset), intesect them
    // otherwise returns either of the non-null set
    match (&set1, &set2) {
      (Some(s1), Some(s2)) => Some(s1.intersection(s2).collect()),
      _ => set1.xor(set2),
    }
  }
}

// we precompute and cache potential_kinds. So patterns should not be mutated.
// Box<[P]> is used here for immutability so that kinds will never be invalidated.
pub struct All<L: Language, P: Matcher<L>> {
  patterns: Box<[P]>,
  kinds: Option<BitSet>,
  lang: PhantomData<L>,
}

impl<L: Language, P: Matcher<L>> All<L, P> {
  pub fn new<PS: IntoIterator<Item = P>>(patterns: PS) -> Self {
    let patterns: Box<[P]> = patterns.into_iter().collect();
    let kinds = Self::compute_kinds(&patterns);
    Self {
      patterns,
      kinds,
      lang: PhantomData,
    }
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

impl<L: Language, P: Matcher<L>> Matcher<L> for All<L, P> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(kinds) = &self.kinds {
      if !kinds.contains(node.kind_id().into()) {
        return None;
      }
    }
    self
      .patterns
      .iter()
      .all(|p| p.match_node_with_env(node.clone(), env).is_some())
      .then_some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.kinds.clone()
  }
}

// Box<[P]> for immutability and potential_kinds cache correctness
pub struct Any<L, P> {
  patterns: Box<[P]>,
  kinds: Option<BitSet>,
  lang: PhantomData<L>,
}

impl<L: Language, P: Matcher<L>> Any<L, P> {
  pub fn new<PS: IntoIterator<Item = P>>(patterns: PS) -> Self {
    let patterns: Box<[P]> = patterns.into_iter().collect();
    let kinds = Self::compute_kinds(&patterns);
    Self {
      patterns,
      kinds,
      lang: PhantomData,
    }
  }

  fn compute_kinds(patterns: &[P]) -> Option<BitSet> {
    let mut set = BitSet::new();
    for pattern in patterns {
      let Some(n) = pattern.potential_kinds() else {
        return None;
      };
      set.union_with(&n);
    }
    Some(set)
  }

  pub fn inner(&self) -> &[P] {
    &self.patterns
  }
}

impl<L: Language, M: Matcher<L>> Matcher<L> for Any<L, M> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
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

pub struct Or<L: Language, P1: Matcher<L>, P2: Matcher<L>> {
  pattern1: P1,
  pattern2: P2,
  lang: PhantomData<L>,
}

impl<L, P1, P2> Matcher<L> for Or<L, P1, P2>
where
  L: Language,
  P1: Matcher<L>,
  P2: Matcher<L>,
{
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
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

pub struct Not<L: Language, M: Matcher<L>> {
  not: M,
  lang: PhantomData<L>,
}

impl<L: Language, M: Matcher<L>> Not<L, M> {
  pub fn new(not: M) -> Self {
    Self {
      not,
      lang: PhantomData,
    }
  }

  pub fn inner(&self) -> &M {
    &self.not
  }
}
impl<L, P> Matcher<L> for Not<L, P>
where
  L: Language,
  P: Matcher<L>,
{
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
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
pub struct Op<L: Language, M: Matcher<L>> {
  inner: M,
  lang: PhantomData<L>,
  // meta_vars: MetaVarMatchers<D>,
}

impl<L, M> Matcher<L> for Op<L, M>
where
  L: Language,
  M: Matcher<L>,
{
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let ret = self.inner.match_node_with_env(node, env);
    ret
    // if ret.is_some() && env.match_constraints(&self.meta_vars) {
    //   ret
    // } else {
    //   None
    // }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.inner.potential_kinds()
  }
}

/*
pub struct Predicate<F> {
  func: F,
}

impl<L, F> Matcher<L> for Predicate<F>
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

impl<L: Language, M: Matcher<L>> Op<L, M> {
  pub fn not(pattern: M) -> Not<L, M> {
    Not {
      not: pattern,
      lang: PhantomData,
    }
  }

  /*
  pub fn with_meta_var(&mut self, var_id: String, matcher: MetaVarMatcher<L>) -> &mut Self {
    self.meta_vars.insert(var_id, matcher);
    self
  }
  */
}

impl<L: Language, M: Matcher<L>> Op<L, M> {
  pub fn every(pattern: M) -> Op<L, And<L, M, MatchAll>> {
    Op {
      inner: And {
        pattern1: pattern,
        pattern2: MatchAll,
        lang: PhantomData,
      },
      lang: PhantomData,
      // meta_vars: MetaVarMatchers::new(),
    }
  }
  pub fn either(pattern: M) -> Op<L, Or<L, M, MatchNone>> {
    Op {
      inner: Or {
        pattern1: pattern,
        pattern2: MatchNone,
        lang: PhantomData,
      },
      lang: PhantomData,
      // meta_vars: MetaVarMatchers::new(),
    }
  }

  pub fn all<MS: IntoIterator<Item = M>>(patterns: MS) -> All<L, M> {
    All::new(patterns)
  }

  pub fn any<MS: IntoIterator<Item = M>>(patterns: MS) -> Any<L, M> {
    Any::new(patterns)
  }

  pub fn new(matcher: M) -> Op<L, M> {
    Self {
      inner: matcher,
      lang: PhantomData,
      // meta_vars: MetaVarMatchers::new(),
    }
  }
}

type NestedAnd<L, M, N, O> = And<L, And<L, M, N>, O>;
impl<L: Language, M: Matcher<L>, N: Matcher<L>> Op<L, And<L, M, N>> {
  pub fn and<O: Matcher<L>>(self, other: O) -> Op<L, NestedAnd<L, M, N, O>> {
    Op {
      inner: And {
        pattern1: self.inner,
        pattern2: other,
        lang: PhantomData,
      },
      lang: PhantomData,
      // meta_vars: MetaVarMatchers::new(),
    }
  }
}

type NestedOr<L, M, N, O> = Or<L, Or<L, M, N>, O>;
impl<L: Language, M: Matcher<L>, N: Matcher<L>> Op<L, Or<L, M, N>> {
  pub fn or<O: Matcher<L>>(self, other: O) -> Op<L, NestedOr<L, M, N, O>> {
    Op {
      inner: Or {
        pattern1: self.inner,
        pattern2: other,
        lang: PhantomData,
      },
      lang: PhantomData,
      // meta_vars: MetaVarMatchers::new(),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::{Root, StrDoc};

  fn test_find(matcher: &impl Matcher<Tsx>, code: &str) {
    let node = Root::str(code, Tsx);
    assert!(matcher.find_node(node.root()).is_some());
  }
  fn test_not_find(matcher: &impl Matcher<Tsx>, code: &str) {
    let node = Root::str(code, Tsx);
    assert!(matcher.find_node(node.root()).is_none());
  }
  fn find_all(matcher: impl Matcher<Tsx>, code: &str) -> Vec<String> {
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
      lang: PhantomData,
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
    let matcher = Not {
      not: "let a = 1",
      lang: PhantomData,
    };
    test_find(&matcher, "const b = 2");
  }

  #[test]
  fn test_and() {
    let matcher = And {
      pattern1: "let a = $_",
      pattern2: Not {
        not: "let a = 123",
        lang: PhantomData,
      },
      lang: PhantomData,
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
    fn t(self) -> Pattern<StrDoc<Tsx>>;
  }
  impl TsxMatcher for &str {
    fn t(self) -> Pattern<StrDoc<Tsx>> {
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

  /*
  use crate::matcher::RegexMatcher;
  #[test]
  fn test_op_with_matchers() {
    let var_matcher = MetaVarMatcher::Regex(RegexMatcher::try_new("a").unwrap());
    let mut matcher = Op::every("$A");
    matcher.with_meta_var("A".into(), var_matcher);
    let code = Root::new("a", Tsx);
    assert!(code.root().find(&matcher).is_some());
    let code = Root::new("b", Tsx);
    assert!(code.root().find(&matcher).is_none());
  }
  */
}
