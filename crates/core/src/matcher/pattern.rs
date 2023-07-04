use crate::language::Language;
use crate::match_tree::{extract_var_from_node, match_end_non_recursive, match_node_non_recursive};
use crate::matcher::{KindMatcher, KindMatcherError, Matcher};
use crate::source::TSParseError;
use crate::{meta_var::MetaVarEnv, Node, Root};
use crate::{Doc, StrDoc};

use bit_set::BitSet;
use std::borrow::Cow;
use thiserror::Error;

/// Pattern style specify how we find the ast node to match, assuming pattern text's root is `Program`
/// the effective AST node to match is either
#[derive(Clone)]
enum PatternStyle<L: Language> {
  /// single non-program ast node, notwithstanding MISSING node.
  Single,
  /// sub AST node specified by user in contextual pattern
  /// e.g. in js`class { $F }` we set selector to public_field_definition
  Selector(KindMatcher<L>),
}

#[derive(Clone)]
pub struct Pattern<D: Doc> {
  pub(crate) root: Root<D>,
  style: PatternStyle<D::Lang>,
}

#[derive(Debug, Error)]
pub enum PatternError {
  #[error("Tree-Sitter fails to parse the pattern.")]
  TSParse(#[from] TSParseError),
  #[error("No AST root is detected. Please check the pattern source `{0}`.")]
  NoContent(String),
  #[error("Multiple AST nodes are detected. Please check the pattern source `{0}`.")]
  MultipleNode(String),
  #[error(transparent)]
  InvalidKind(#[from] KindMatcherError),
  #[error("Fails to create Contextual pattern: selector `{selector}` matches no node in the context `{context}`.")]
  NoSelectorInContext { context: String, selector: String },
}

#[inline]
fn is_single_node(n: &tree_sitter::Node) -> bool {
  match n.child_count() {
    1 => true,
    2 => n.child(1).expect("second child must exist").is_missing(),
    _ => false,
  }
}
impl<L: Language> Pattern<StrDoc<L>> {
  pub fn str(src: &str, lang: L) -> Self {
    Self::new(src, lang)
  }

  pub fn fixed_string(&self) -> Cow<str> {
    let node = match &self.style {
      PatternStyle::Single => self.single_matcher(),
      PatternStyle::Selector(kind) => self.kind_matcher(kind),
    };
    let mut fixed = Cow::Borrowed("");
    for n in node.dfs() {
      if n.is_leaf() && extract_var_from_node(&n).is_none() && n.text().len() > fixed.len() {
        fixed = n.text();
      }
    }
    fixed
  }
}

impl<D: Doc> Pattern<D> {
  pub fn try_new(src: &str, lang: D::Lang) -> Result<Self, PatternError> {
    let processed = lang.pre_process_pattern(src);
    let root = Root::try_new(&processed, lang)?;
    let goal = root.root();
    if goal.inner.child_count() == 0 {
      return Err(PatternError::NoContent(src.into()));
    }
    if !is_single_node(&goal.inner) {
      return Err(PatternError::MultipleNode(src.into()));
    }
    Ok(Self {
      root,
      style: PatternStyle::Single,
    })
  }

  pub fn new(src: &str, lang: D::Lang) -> Self {
    Self::try_new(src, lang).unwrap()
  }

  pub fn contextual(context: &str, selector: &str, lang: D::Lang) -> Result<Self, PatternError> {
    let processed = lang.pre_process_pattern(context);
    let root = Root::try_new(&processed, lang.clone())?;
    let goal = root.root();
    let kind_matcher = KindMatcher::try_new(selector, lang)?;
    if goal.find(&kind_matcher).is_none() {
      return Err(PatternError::NoSelectorInContext {
        context: context.into(),
        selector: selector.into(),
      });
    }
    Ok(Self {
      root,
      style: PatternStyle::Selector(kind_matcher),
    })
  }
  pub fn doc(doc: D) -> Self {
    Self {
      root: Root::doc(doc),
      style: PatternStyle::Single,
    }
  }
  fn single_matcher(&self) -> Node<D> {
    debug_assert!(matches!(self.style, PatternStyle::Single));
    let root = self.root.root();
    let mut node = root.inner;
    while is_single_node(&node) {
      node = node.child(0).unwrap();
    }
    Node {
      inner: node,
      root: &self.root,
    }
  }

  fn kind_matcher(&self, kind_matcher: &KindMatcher<D::Lang>) -> Node<D> {
    debug_assert!(matches!(self.style, PatternStyle::Selector(_)));
    self
      .root
      .root()
      .find(kind_matcher)
      .map(Node::from)
      .expect("contextual match should succeed")
  }
}

impl<P: Doc> Matcher<P::Lang> for Pattern<P> {
  fn match_node_with_env<'tree, D: Doc<Lang = P::Lang>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    match &self.style {
      PatternStyle::Single => {
        let matcher = self.single_matcher();
        match_node_non_recursive(&matcher, node, env)
      }
      PatternStyle::Selector(kind) => {
        let matcher = self.kind_matcher(kind);
        match_node_non_recursive(&matcher, node, env)
      }
    }
  }

  fn potential_kinds(&self) -> Option<bit_set::BitSet> {
    let kind = match &self.style {
      PatternStyle::Selector(kind) => return kind.potential_kinds(),
      PatternStyle::Single => {
        let matcher = self.single_matcher();
        if matcher.is_leaf() && extract_var_from_node(&matcher).is_some() {
          return None;
        }
        matcher.kind_id()
      }
    };
    let mut kinds = BitSet::new();
    kinds.insert(kind.into());
    Some(kinds)
  }

  fn get_match_len<D: Doc<Lang = P::Lang>>(&self, node: Node<D>) -> Option<usize> {
    let start = node.range().start;
    let end = match &self.style {
      PatternStyle::Single => match_end_non_recursive(&self.single_matcher(), node)?,
      PatternStyle::Selector(kind) => match_end_non_recursive(&self.kind_matcher(kind), node)?,
    };
    Some(end - start)
  }
}

impl<D: Doc> std::fmt::Debug for Pattern<D> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.style {
      PatternStyle::Single => write!(f, "{}", self.single_matcher().to_sexp()),
      PatternStyle::Selector(kind) => write!(f, "{}", self.kind_matcher(kind).to_sexp()),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use std::collections::HashMap;

  fn pattern_node(s: &str) -> Root<StrDoc<Tsx>> {
    Root::new(s, Tsx)
  }

  fn test_match(s1: &str, s2: &str) {
    let pattern = Pattern::str(s1, Tsx);
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
    let pattern = Pattern::str(s1, Tsx);
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
    let pattern = Pattern::str(goal_str, Tsx);
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
    let pattern: Pattern<StrDoc<_>> =
      Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx).expect("test");
    let cand = pattern_node("class B { b = 123 }");
    assert!(pattern.find_node(cand.root()).is_some());
    let cand = pattern_node("let b = 123");
    assert!(pattern.find_node(cand.root()).is_none());
  }

  #[test]
  fn test_contextual_match_with_env() {
    let pattern: Pattern<StrDoc<_>> =
      Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx).expect("test");
    let cand = pattern_node("class B { b = 123 }");
    let nm = pattern.find_node(cand.root()).expect("test");
    let env = nm.get_env();
    let env = HashMap::from(env.clone());
    assert_eq!(env["F"], "b");
    assert_eq!(env["I"], "123");
  }

  #[test]
  fn test_contextual_unmatch_with_env() {
    let pattern: Pattern<StrDoc<_>> =
      Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx).expect("test");
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
    let pattern = Pattern::str("const a = 1", Tsx);
    let kind = get_kind("lexical_declaration");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_pattern_with_non_root_meta_var() {
    let pattern = Pattern::str("const $A = $B", Tsx);
    let kind = get_kind("lexical_declaration");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_bare_wildcard() {
    let pattern = Pattern::str("$A", Tsx);
    // wildcard should match anything, so kinds should be None
    assert!(pattern.potential_kinds().is_none());
  }

  #[test]
  fn test_contextual_potential_kinds() {
    let pattern: Pattern<StrDoc<_>> =
      Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx).expect("test");
    let kind = get_kind("public_field_definition");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_contextual_wildcard() {
    let pattern =
      Pattern::<StrDoc<_>>::contextual("class A { $F }", "property_identifier", Tsx).expect("test");
    let kind = get_kind("property_identifier");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  #[ignore]
  fn test_multi_node_pattern() {
    let pattern = Pattern::str("a;b;c;", Tsx);
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    test_match("a;b;c", "a;b;c;");
  }

  #[test]
  #[ignore]
  fn test_multi_node_meta_var() {
    let env = match_env("a;$B;c", "a;b;c");
    assert_eq!(env["B"], "b");
    let env = match_env("a;$B;c", "a;1+2+3;c");
    assert_eq!(env["B"], "1+2+3");
  }

  #[test]
  #[ignore]
  fn test_pattern_size() {
    assert_eq!(std::mem::size_of::<Pattern<StrDoc<Tsx>>>(), 40);
  }
}
