use crate::language::Language;
use crate::match_tree::{match_end_non_recursive, match_node_non_recursive, MatchStrictness};
use crate::matcher::{kind_utils, KindMatcher, KindMatcherError, Matcher};
use crate::meta_var::{MetaVarEnv, MetaVariable};
use crate::source::TSParseError;
use crate::{Doc, Node, Root, StrDoc};

use bit_set::BitSet;
use thiserror::Error;

use std::borrow::Cow;
use std::collections::HashSet;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Pattern<L: Language> {
  pub node: PatternNode,
  root_kind: Option<u16>,
  lang: PhantomData<L>,
  pub strictness: MatchStrictness,
}

#[derive(Clone)]
pub enum PatternNode {
  MetaVar {
    meta_var: MetaVariable,
  },
  /// Node without children.
  Terminal {
    text: String,
    is_named: bool,
    kind_id: u16,
  },
  /// Non-Terminal Syntax Nodes are called Internal
  Internal {
    kind_id: u16,
    children: Vec<PatternNode>,
  },
}

impl PatternNode {
  // for skipping trivial nodes in goal after ellipsis
  pub fn is_trivial(&self) -> bool {
    match self {
      PatternNode::Terminal { is_named, .. } => !*is_named,
      _ => false,
    }
  }

  pub fn fixed_string(&self) -> Cow<str> {
    match &self {
      PatternNode::Terminal { text, .. } => Cow::Borrowed(text),
      PatternNode::MetaVar { .. } => Cow::Borrowed(""),
      PatternNode::Internal { children, .. } => {
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
}
impl<'r, D: Doc> From<Node<'r, D>> for PatternNode {
  fn from(node: Node<'r, D>) -> Self {
    convert_node_to_pattern(node)
  }
}

impl<'r, D: Doc> From<Node<'r, D>> for Pattern<D::Lang> {
  fn from(node: Node<'r, D>) -> Self {
    Self {
      node: convert_node_to_pattern(node),
      root_kind: None,
      lang: PhantomData,
      strictness: MatchStrictness::Smart,
    }
  }
}

fn convert_node_to_pattern<D: Doc>(node: Node<D>) -> PatternNode {
  if let Some(meta_var) = extract_var_from_node(&node) {
    PatternNode::MetaVar { meta_var }
  } else if node.is_leaf() {
    PatternNode::Terminal {
      text: node.text().to_string(),
      is_named: node.is_named(),
      kind_id: node.kind_id(),
    }
  } else {
    let children = node.children().filter_map(|n| {
      if n.get_ts_node().is_missing() {
        None
      } else {
        Some(PatternNode::from(n))
      }
    });
    PatternNode::Internal {
      kind_id: node.kind_id(),
      children: children.collect(),
    }
  }
}

fn extract_var_from_node<D: Doc>(goal: &Node<D>) -> Option<MetaVariable> {
  let key = goal.text();
  goal.lang().extract_meta_var(&key)
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
impl<L: Language> Pattern<L> {
  pub fn str(src: &str, lang: L) -> Self {
    Self::new(src, lang)
  }

  pub fn has_error(&self) -> bool {
    let kind = match &self.node {
      PatternNode::Terminal { kind_id, .. } => *kind_id,
      PatternNode::Internal { kind_id, .. } => *kind_id,
      PatternNode::MetaVar { .. } => match self.root_kind {
        Some(k) => k,
        None => return false,
      },
    };
    kind_utils::is_error_kind(kind)
  }

  pub fn fixed_string(&self) -> Cow<str> {
    self.node.fixed_string()
  }

  /// Get all defined variables in the pattern.
  /// Used for validating rules and report undefined variables.
  pub fn defined_vars(&self) -> HashSet<&str> {
    let mut vars = HashSet::new();
    collect_vars(&self.node, &mut vars);
    vars
  }
}

fn meta_var_name(meta_var: &MetaVariable) -> Option<&str> {
  use MetaVariable as MV;
  match meta_var {
    MV::Capture(name, _) => Some(name),
    MV::MultiCapture(name) => Some(name),
    MV::Dropped(_) => None,
    MV::Multiple => None,
  }
}

fn collect_vars<'p>(p: &'p PatternNode, vars: &mut HashSet<&'p str>) {
  match p {
    PatternNode::MetaVar { meta_var, .. } => {
      if let Some(name) = meta_var_name(meta_var) {
        vars.insert(name);
      }
    }
    PatternNode::Terminal { .. } => {
      // collect nothing for terminal nodes!
    }
    PatternNode::Internal { children, .. } => {
      for c in children {
        collect_vars(c, vars);
      }
    }
  }
}

impl<L: Language> Pattern<L> {
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

  pub fn with_strictness(mut self, strictness: MatchStrictness) -> Self {
    self.strictness = strictness;
    self
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
    Ok(Self {
      root_kind: Some(node.kind_id()),
      node: convert_node_to_pattern(node.get_node().clone()),
      lang: PhantomData,
      strictness: MatchStrictness::Smart,
    })
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
}

impl<L: Language> Matcher<L> for Pattern<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(k) = self.root_kind {
      if node.kind_id() != k {
        return None;
      }
    }
    // do not pollute the env if pattern does not match
    let mut may_write = Cow::Borrowed(env.as_ref());
    let node = match_node_non_recursive(self, node, &mut may_write)?;
    if let Cow::Owned(map) = may_write {
      // only change env when pattern matches
      *env = Cow::Owned(map);
    }
    Some(node)
  }

  fn potential_kinds(&self) -> Option<bit_set::BitSet> {
    let kind = match self.node {
      PatternNode::Terminal { kind_id, .. } => kind_id,
      PatternNode::MetaVar { .. } => self.root_kind?,
      PatternNode::Internal { kind_id, .. } => {
        if kind_utils::is_error_kind(kind_id) {
          // error can match any kind
          return None;
        }
        kind_id
      }
    };

    let mut kinds = BitSet::new();
    kinds.insert(kind.into());
    Some(kinds)
  }

  fn get_match_len<D: Doc<Lang = L>>(&self, node: Node<D>) -> Option<usize> {
    let start = node.range().start;
    let end = match_end_non_recursive(self, node)?;
    Some(end - start)
  }
}
impl std::fmt::Debug for PatternNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MetaVar { meta_var, .. } => write!(f, "{:?}", meta_var),
      Self::Terminal { text, .. } => write!(f, "{}", text),
      Self::Internal { children, .. } => write!(f, "{:?}", children),
    }
  }
}

impl<L: Language> std::fmt::Debug for Pattern<L> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.node)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::matcher::MatcherExt;
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

  #[test]
  fn test_whitespace() {
    test_match("function t() { }", "function t() {}");
    test_match("function t() {}", "function t() {  }");
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
  fn test_pattern_should_not_pollute_env() {
    // gh issue #1164
    let pattern = Pattern::str("const $A = 114", Tsx);
    let cand = pattern_node("const a = 514");
    let cand = cand.root().child(0).unwrap();
    let map = MetaVarEnv::new();
    let mut env = Cow::Borrowed(&map);
    let nm = pattern.match_node_with_env(cand, &mut env);
    assert!(nm.is_none());
    assert!(env.get_match("A").is_none());
    assert!(map.get_match("A").is_none());
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
    assert_eq!(std::mem::size_of::<Pattern<Tsx>>(), 40);
  }

  #[test]
  fn test_doc_pattern() {
    let doc = StrDoc::new("let a = 123", Tsx);
    let pattern = Pattern::doc(doc);
    let kinds = pattern.potential_kinds().expect("should have kinds");
    assert_eq!(kinds.len(), 1);
  }

  #[test]
  fn test_error_kind() {
    let ret = Pattern::contextual("a", "property_identifier", Tsx);
    assert!(ret.is_err());
    let ret = Pattern::str("123+", Tsx);
    assert!(ret.has_error());
  }

  #[test]
  fn test_bare_wildcard_in_context() {
    let pattern = Pattern::contextual("class A { $F }", "property_identifier", Tsx).expect("test");
    let cand = pattern_node("let b = 123");
    // it should not match
    assert!(pattern.find_node(cand.root()).is_none());
  }

  #[test]
  fn test_pattern_fixed_string() {
    let pattern = Pattern::new("class A { $F }", Tsx);
    assert_eq!(pattern.fixed_string(), "class");
    let pattern = Pattern::contextual("class A { $F }", "property_identifier", Tsx).expect("test");
    assert!(pattern.fixed_string().is_empty());
  }

  #[test]
  fn test_pattern_error() {
    let pattern = Pattern::try_new("", Tsx);
    assert!(matches!(pattern, Err(PatternError::NoContent(_))));
    let pattern = Pattern::try_new("12  3344", Tsx);
    assert!(matches!(pattern, Err(PatternError::MultipleNode(_))));
  }

  #[test]
  fn test_debug_pattern() {
    let pattern = Pattern::str("var $A = 1", Tsx);
    assert_eq!(
      format!("{pattern:?}"),
      "[var, [Capture(\"A\", true), =, 1]]"
    );
  }

  fn defined_vars(s: &str) -> Vec<String> {
    let pattern = Pattern::str(s, Tsx);
    let mut vars: Vec<_> = pattern
      .defined_vars()
      .into_iter()
      .map(String::from)
      .collect();
    vars.sort();
    vars
  }

  #[test]
  fn test_extract_meta_var_from_pattern() {
    let vars = defined_vars("var $A = 1");
    assert_eq!(vars, ["A"]);
  }

  #[test]
  fn test_extract_complex_meta_var() {
    let vars = defined_vars("function $FUNC($$$ARGS): $RET { $$$BODY }");
    assert_eq!(vars, ["ARGS", "BODY", "FUNC", "RET"]);
  }

  #[test]
  fn test_extract_duplicate_meta_var() {
    let vars = defined_vars("var $A = $A");
    assert_eq!(vars, ["A"]);
  }

  #[test]
  fn test_contextual_pattern_vars() {
    let pattern = Pattern::contextual("<div ref={$A}/>", "jsx_attribute", Tsx).expect("correct");
    assert_eq!(pattern.defined_vars(), ["A"].into_iter().collect());
  }

  #[test]
  fn test_gh_1087() {
    test_match("($P) => $F($P)", "(x) => bar(x)");
  }
}
