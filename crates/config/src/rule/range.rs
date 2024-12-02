use ast_grep_core::{meta_var::MetaVarEnv, Doc, Language, Node, Position};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents a position in a document
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializablePosition {
  pub row: usize,
  pub column: usize,
}

/// Represents a position in source code using 0-based row and column numbers
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRange {
  /// 0-based row number in the source code
  pub start: SerializablePosition,
  /// 0-based column number in the source code
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
  /// - positions contain invalid row/column values
  #[error("The supplied start position must be before the end position.")]
  InvalidRange,
}

impl SerializablePosition {
  pub fn equals_node_pos<D: Doc>(&self, pos: &Position, node: &Node<D>) -> bool {
    let row = pos.row();
    let column = pos.column(node);
    self.row == row && self.column == column
  }
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
    if start_pos.row > end_pos.row
      || (start_pos.row == end_pos.row && start_pos.column > end_pos.column)
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

    if self.start.equals_node_pos(&node_start_pos, &node)
      && self.end.equals_node_pos(&node_end_pos, &node)
    {
      Some(node)
    } else {
      None
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript as TS;

  #[test]
  fn test_invalid_range() {
    let range = RangeMatcher::<TS>::try_new(
      SerializablePosition { row: 0, column: 10 },
      SerializablePosition { row: 0, column: 5 },
    );
    assert!(range.is_err());
  }

  #[test]
  fn test_range_match() {
    let cand = TS::Tsx.ast_grep("class A { a = 123 }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition { row: 0, column: 10 },
      SerializablePosition { row: 0, column: 17 },
    );
    assert!(
      pattern.find_node(cand.clone()).is_some(),
      "goal: {}, candidate: {}",
      "public_field_definition",
      cand.to_sexp(),
    );
  }

  #[test]
  fn test_range_non_match() {
    let cand = TS::Tsx.ast_grep("class A { a = 123 }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition { row: 0, column: 10 },
      SerializablePosition { row: 0, column: 15 },
    );
    assert!(
      pattern.find_node(cand.clone()).is_none(),
      "goal: {}, candidate: {}",
      "public_field_definition",
      cand.to_sexp(),
    );
  }

  #[test]
  fn test_multiline_range() {
    let cand = TS::Tsx
      .ast_grep("class A { \n b = () => { \n const c = 1 \n const d = 3 \n return c + d \n } }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(
      SerializablePosition { row: 1, column: 1 },
      SerializablePosition { row: 5, column: 2 },
    );
    assert!(
      pattern.find_node(cand.clone()).is_some(),
      "goal: {}, candidate: {}",
      "public_field_definition",
      cand.to_sexp(),
    );
  }
}
