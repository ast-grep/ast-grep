use super::Matcher;

use crate::language::Language;
use crate::meta_var::MetaVarEnv;
use crate::node::KindId;
use crate::{Doc, Node};

use std::borrow::Cow;

use bit_set::BitSet;
use thiserror::Error;

// 0 is symbol_end for not found, 65535 is builtin symbol ERROR
// see https://tree-sitter.docsforge.com/master/api/#TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION
// and https://tree-sitter.docsforge.com/master/api/ts_language_symbol_for_name/
const TS_BUILTIN_SYM_END: KindId = 0;
const TS_BUILTIN_SYM_ERROR: KindId = 65535;

#[derive(Debug, Error)]
pub enum KindMatcherError {
  #[error("Kind `{0}` is invalid.")]
  InvalidKindName(String),
}

#[derive(Clone)]
pub struct KindMatcher {
  kind: KindId,
}

impl KindMatcher {
  pub fn new<L: Language>(node_kind: &str, lang: L) -> Self {
    Self {
      kind: lang.kind_to_id(node_kind),
    }
  }

  pub fn try_new<L: Language>(node_kind: &str, lang: L) -> Result<Self, KindMatcherError> {
    let s = Self::new(node_kind, lang);
    if s.is_invalid() {
      Err(KindMatcherError::InvalidKindName(node_kind.into()))
    } else {
      Ok(s)
    }
  }

  pub fn from_id(kind: KindId) -> Self {
    Self { kind }
  }

  /// Whether the kind matcher contains undefined tree-sitter kind.
  pub fn is_invalid(&self) -> bool {
    self.kind == TS_BUILTIN_SYM_END
  }

  /// Construct a matcher that only matches ERROR
  pub fn error_matcher() -> Self {
    Self::from_id(TS_BUILTIN_SYM_ERROR)
  }
}

pub mod kind_utils {
  use super::*;

  /// Whether the kind will match parsing error occurred in the source code.
  /// for example, we can use `kind: ERROR` in YAML to find invalid syntax in source.
  /// the name `is_error` implies the matcher itself is error.
  /// But here the matcher itself is valid and it is what it matches is error.
  pub fn is_error_kind(kind: KindId) -> bool {
    kind == TS_BUILTIN_SYM_ERROR
  }

  pub fn are_kinds_matching(goal: KindId, candidate: KindId) -> bool {
    goal == candidate || is_error_kind(goal)
  }
}

impl Matcher for KindMatcher {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if node.kind_id() == self.kind {
      Some(node)
    } else {
      None
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    let mut set = BitSet::new();
    set.insert(self.kind.into());
    Some(set)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::matcher::MatcherExt;
  use crate::{tree_sitter::StrDoc, Root};

  fn pattern_node(s: &str) -> Root<StrDoc<Tsx>> {
    Root::str(s, Tsx)
  }
  #[test]
  fn test_kind_match() {
    let kind = "public_field_definition";
    let cand = pattern_node("class A { a = 123 }");
    let cand = cand.root();
    let pattern = KindMatcher::new(kind, Tsx);
    assert!(
      pattern.find_node(cand.clone()).is_some(),
      "goal: {}, candidate: {}",
      kind,
      cand.get_inner_node().to_sexp(),
    );
  }

  #[test]
  fn test_kind_non_match() {
    let kind = "field_definition";
    let cand = pattern_node("const a = 123");
    let cand = cand.root();
    let pattern = KindMatcher::new(kind, Tsx);
    assert!(
      pattern.find_node(cand.clone()).is_none(),
      "goal: {}, candidate: {}",
      kind,
      cand.get_inner_node().to_sexp(),
    );
  }

  #[test]
  fn test_kind_potential_kinds() {
    let kind = "field_definition";
    let matcher = KindMatcher::new(kind, Tsx);
    let potential_kinds = matcher
      .potential_kinds()
      .expect("should have potential kinds");
    // should has exactly one potential kind
    assert_eq!(potential_kinds.len(), 1);
  }
}
