use crate::language::Language;
use crate::match_tree::{extract_var_from_node, match_end_non_recursive, match_node_non_recursive};
use crate::matcher::{KindMatcher, KindMatcherError, Matcher};
use crate::meta_var::{MetaVarEnv, MetaVariable};
use crate::source::TSParseError;
use crate::{Doc, Node, Root, StrDoc};

use bit_set::BitSet;
use std::borrow::Cow;
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Clone)]
pub enum Pattern<D: Doc> {
  MetaVar(MetaVariable),
  Leaf {
    text: String,
    is_named: bool,
    kind_id: u16,
    lang: PhantomData<D>,
  },
  NonTerminal {
    kind_id: u16,
    children: Vec<Pattern<D>>,
  },
}

impl<'r, D: Doc> From<Node<'r, D>> for Pattern<D> {
  fn from(node: Node<'r, D>) -> Self {
    if let Some(meta_var) = extract_var_from_node(&node) {
      Self::MetaVar(meta_var)
    } else if node.is_leaf() {
      Self::Leaf {
        text: node.text().to_string(),
        is_named: node.is_named(),
        kind_id: node.kind_id(),
        lang: PhantomData,
      }
    } else {
      Self::NonTerminal {
        kind_id: node.kind_id(),
        children: node.children().map(Self::from).collect(),
      }
    }
  }
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
    2 => {
      let c = n.child(1).expect("second child must exist");
      // some language will have weird empty syntax node at the end
      // see golang's `$A = 0` pattern test case
      c.is_missing() || c.kind().is_empty()
    }
    _ => false,
  }
}
impl<L: Language> Pattern<StrDoc<L>> {
  pub fn str(src: &str, lang: L) -> Self {
    Self::new(src, lang)
  }

  pub fn fixed_string(&self) -> Cow<str> {
    match self {
      Self::Leaf { text, .. } => Cow::Borrowed(text),
      Self::MetaVar(_) => Cow::Borrowed(""),
      Self::NonTerminal { children, .. } => {
        children
          .iter()
          .map(|n| n.fixed_string())
          .fold(Cow::Borrowed(""), |longest, curr| {
            if longest.len() >= curr.len() {
              longest
            } else {
              curr
            }
          })
      }
    }
  }

  pub fn has_error(&self) -> bool {
    todo!("pattern")
    // let node = match &self.style {
    //   PatternStyle::Single => self.single_matcher(),
    //   PatternStyle::Selector(kind) => self.kind_matcher(kind),
    // };
    // node.matches(KindMatcher::error_matcher())
  }
}

impl<L: Language> Pattern<StrDoc<L>> {
  pub fn try_new(src: &str, lang: L) -> Result<Self, PatternError> {
    let processed = lang.pre_process_pattern(src);
    let root = Root::<StrDoc<L>>::try_new(&processed, lang)?;
    let goal = root.root();
    if goal.inner.child_count() == 0 {
      return Err(PatternError::NoContent(src.into()));
    }
    if !is_single_node(&goal.inner) {
      return Err(PatternError::MultipleNode(src.into()));
    }
    let node = Self::single_matcher(&root);
    Ok(Self::from(node))
  }

  pub fn new(src: &str, lang: L) -> Self {
    Self::try_new(src, lang).unwrap()
  }

  pub fn contextual(context: &str, selector: &str, lang: L) -> Result<Self, PatternError> {
    let processed = lang.pre_process_pattern(context);
    let root = Root::<StrDoc<L>>::try_new(&processed, lang.clone())?;
    let goal = root.root();
    let kind_matcher = KindMatcher::try_new(selector, lang)?;
    let Some(node) = goal.find(&kind_matcher) else {
      return Err(PatternError::NoSelectorInContext {
        context: context.into(),
        selector: selector.into(),
      });
    };
    Ok(Self::from(node.get_node().clone()))
  }
  pub fn doc(doc: StrDoc<L>) -> Self {
    let root = Root::doc(doc);
    Self::from(root.root())
  }
  fn single_matcher<D: Doc>(root: &Root<D>) -> Node<D> {
    // debug_assert!(matches!(self.style, PatternStyle::Single));
    let node = root.root();
    let mut inner = node.inner;
    while is_single_node(&inner) {
      inner = inner.child(0).unwrap();
    }
    Node { inner, root }
  }

  fn kind_matcher<D: Doc>(&self, kind_matcher: &KindMatcher<D::Lang>) -> Node<D> {
    todo!("pattern")
    // debug_assert!(matches!(self.style, PatternStyle::Selector(_)));
    // self
    //   .root
    //   .root()
    //   .find(kind_matcher)
    //   .map(Node::from)
    //   .expect("contextual match should succeed")
  }
}

impl<L: Language, P: Doc<Lang = L>> Matcher<L> for Pattern<P> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    todo!("pattern")
    // match_node_non_recursive(&self, node, env)
  }

  fn potential_kinds(&self) -> Option<bit_set::BitSet> {
    let kind = match self {
      Self::Leaf { kind_id, .. } => *kind_id,
      Self::MetaVar(_) => return None,
      Self::NonTerminal { kind_id, .. } => *kind_id,
    };
    let mut kinds = BitSet::new();
    kinds.insert(kind.into());
    Some(kinds)
  }

  fn get_match_len<D: Doc<Lang = L>>(&self, node: Node<D>) -> Option<usize> {
    todo!("pattern")
    // let start = node.range().start;
    // let end = match_end_non_recursive(self, node)?;
    // Some(end - start)
  }
}

impl<D: Doc> std::fmt::Debug for Pattern<D> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MetaVar(m) => write!(f, "{:?}", m),
      Self::Leaf { text, .. } => write!(f, "{}", text),
      Self::NonTerminal { children, .. } => write!(f, "{:?}", children),
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
      "goal: {:?}, candidate: {}",
      pattern,
      cand.to_sexp(),
    );
  }
  fn test_non_match(s1: &str, s2: &str) {
    let pattern = Pattern::str(s1, Tsx);
    let cand = pattern_node(s2);
    let cand = cand.root();
    assert!(
      pattern.find_node(cand.clone()).is_none(),
      "goal: {:?}, candidate: {}",
      pattern,
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
    let pattern =
      Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx).expect("test");
    let cand = pattern_node("class B { b = 123 }");
    assert!(pattern.find_node(cand.root()).is_some());
    let cand = pattern_node("let b = 123");
    assert!(pattern.find_node(cand.root()).is_none());
  }

  #[test]
  fn test_contextual_match_with_env() {
    let pattern =
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
    let pattern =
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
    let pattern =
      Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx).expect("test");
    let kind = get_kind("public_field_definition");
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(kind));
  }

  #[test]
  fn test_contextual_wildcard() {
    let pattern = Pattern::contextual("class A { $F }", "property_identifier", Tsx).expect("test");
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

  #[test]
  fn test_doc_pattern() {
    let doc = StrDoc::new("let a = 123", Tsx);
    let pattern = Pattern::doc(doc);
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
  }

  #[test]
  fn test_error() {
    let ret = Pattern::contextual("a", "property_identifier", Tsx);
    assert!(ret.is_err());
    let ret = Pattern::str("123+", Tsx);
    assert!(ret.has_error());
  }

  #[test]
  fn test_bare_wildcard_in_context() {
    let pattern =
      Pattern::<StrDoc<_>>::contextual("class A { $F }", "property_identifier", Tsx).expect("test");
    let cand = pattern_node("let b = 123");
    // should it match?
    assert!(pattern.find_node(cand.root()).is_some());
  }
}
