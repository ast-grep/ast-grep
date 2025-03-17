use ast_grep_core::{meta_var::MetaVarEnv, Doc, Language, Node};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents a zero-based character-wise position in a document
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializablePosition {
  /// 0-based line number in the source code
  pub line: usize,
  /// 0-based column number in the source code
  pub column: usize,
}

/// Represents a position in source code using 0-based line and column numbers
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRange {
  /// start position in the source code
  pub start: SerializablePosition,
  /// end position in the source code
  pub end: SerializablePosition,
}

use std::{borrow::Cow, marker::PhantomData};

use bit_set::BitSet;
use thiserror::Error;

use super::Matcher;

/// Errors that can occur when creating or using a RangeMatcher
#[derive(Debug, Error)]
pub enum RangeMatcherError {
  /// Returned when the range is invalid. This can occur when:
  /// - start position is after end position
  /// - positions contain invalid line/column values
  #[error("The start position must be before the end position.")]
  InvalidRange,
}

pub struct RangeMatcher<L: Language> {
  start: SerializablePosition,
  end: SerializablePosition,
  lang: PhantomData<L>,
}

impl<L: Language> RangeMatcher<L> {
  pub fn new(start_pos: SerializablePosition, end_pos: SerializablePosition) -> Self {
    Self {
      start: start_pos,
      end: end_pos,
      lang: PhantomData,
    }
  }

  pub fn try_new(
    start_pos: SerializablePosition,
    end_pos: SerializablePosition,
  ) -> Result<RangeMatcher<L>, RangeMatcherError> {
    if start_pos.line > end_pos.line
      || (start_pos.line == end_pos.line && start_pos.column > end_pos.column)
    {
      return Err(RangeMatcherError::InvalidRange);
    }

    let range = Self::new(start_pos, end_pos);
    Ok(range)
  }
}

impl<L: Language> Matcher<L> for RangeMatcher<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let node_start_pos = node.start_pos();
    let node_end_pos = node.end_pos();

    // first check line since it is cheaper
    if self.start.line != node_start_pos.line() || self.end.line != node_end_pos.line() {
      return None;
    }
    // then check column, this can be expensive for utf-8 encoded files
    if self.start.column != node_start_pos.column(&node)
      || self.end.column != node_end_pos.column(&node)
    {
      return None;
    }
    Some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript as TS;
  use ast_grep_core::matcher::MatcherExt;

  #[test]
  fn test_invalid_range() {
    let range = RangeMatcher::<TS>::try_new(
      SerializablePosition {
        line: 0,
        column: 10,
      },
      SerializablePosition { line: 0, column: 5 },
    );
    assert!(range.is_err());
  }

  #[test]
  fn test_range_match() {
    let cand = TS::Tsx.ast_grep("class A { a = 123 }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition {
        line: 0,
        column: 10,
      },
      SerializablePosition {
        line: 0,
        column: 17,
      },
    );
    assert!(pattern.find_node(cand).is_some());
  }

  #[test]
  fn test_range_non_match() {
    let cand = TS::Tsx.ast_grep("class A { a = 123 }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition {
        line: 0,
        column: 10,
      },
      SerializablePosition {
        line: 0,
        column: 15,
      },
    );
    assert!(pattern.find_node(cand).is_none(),);
  }

  #[test]
  fn test_multiline_range() {
    let cand = TS::Tsx
      .ast_grep("class A { \n b = () => { \n const c = 1 \n const d = 3 \n return c + d \n } }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition { line: 1, column: 1 },
      SerializablePosition { line: 5, column: 2 },
    );
    assert!(pattern.find_node(cand).is_some());
  }

  #[test]
  fn test_unicode_range() {
    let cand = TS::Tsx.ast_grep("let a = '🦄'");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition { line: 0, column: 8 },
      SerializablePosition {
        line: 0,
        column: 11,
      },
    );
    let node = pattern.find_node(cand);
    assert!(node.is_some());
    assert_eq!(node.expect("should exist").text(), "'🦄'");
  }
}
