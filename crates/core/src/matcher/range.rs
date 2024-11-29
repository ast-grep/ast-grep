use std::{borrow::Cow, marker::PhantomData};

use bit_set::BitSet;
use thiserror::Error;

use crate::{meta_var::MetaVarEnv, Doc, Language, Node};

use super::Matcher;

#[derive(Debug, Error)]
pub enum RangeMatcherError {
  #[error("Range is invalid.")]
  InvalidRange,
}

pub struct Position<L: Language> {
  row: usize,
  column: usize,
  lang: PhantomData<L>,
}

pub struct RangeMatcher<L: Language> {
  start: Position<L>,
  end: Position<L>,
}

impl<L: Language> RangeMatcher<L> {
  pub fn new(start_row: usize, start_column: usize, end_row: usize, end_column: usize) -> Self {
    Self {
      start: Position {
        row: start_row,
        column: start_column,
        lang: PhantomData,
      },
      end: Position {
        row: end_row,
        column: end_column,
        lang: PhantomData,
      },
    }
  }

  pub fn try_new(
    start_row: usize,
    start_column: usize,
    end_row: usize,
    end_column: usize,
  ) -> Result<Self, RangeMatcherError> {
    let range = Self::new(start_row, start_column, end_row, end_column);
    Ok(range)
  }
}

impl<L: Language> Matcher<L> for RangeMatcher<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let (start_row, start_column) = node.start_pos();
    let (end_row, end_column) = node.end_pos();
    if start_row == self.start.row
      && start_column == self.start.column
      && end_row == self.end.row
      && end_column == self.end.column
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
  use crate::language::Tsx;
  use crate::{Root, StrDoc};

  fn pattern_node(s: &str) -> Root<StrDoc<Tsx>> {
    Root::new(s, Tsx)
  }

  #[test]
  fn test_range_match() {
    let cand = pattern_node("class A { a = 123 }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(0, 10, 0, 17);
    assert!(
      pattern.find_node(cand.clone()).is_some(),
      "goal: {}, candidate: {}",
      "public_field_definition",
      cand.to_sexp(),
    );
  }

  #[test]
  fn test_range_non_match() {
    let cand = pattern_node("class A { a = 123 }");
    let cand = cand.root();
    let pattern = RangeMatcher::new(0, 10, 0, 15);
    assert!(
      pattern.find_node(cand.clone()).is_none(),
      "goal: {}, candidate: {}",
      "public_field_definition",
      cand.to_sexp(),
    );
  }
}
