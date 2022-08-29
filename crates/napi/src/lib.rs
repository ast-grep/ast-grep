#![deny(clippy::all)]

use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::{NodeMatch, Pattern};
use napi_derive::napi;

#[derive(Clone)]
pub struct Tsx;
impl Language for Tsx {
  fn get_ts_language(&self) -> TSLanguage {
    tree_sitter_typescript::language_tsx().into()
  }
}

#[napi(object)]
pub struct Pos {
  pub row: u32,
  pub col: u32,
}
fn from_tuple(pos: (usize, usize)) -> Pos {
  Pos {
    row: pos.0 as u32,
    col: pos.1 as u32,
  }
}

#[napi(object)]
pub struct MatchResult {
  pub start: Pos,
  pub end: Pos,
}

impl<L: Language> From<NodeMatch<'_, L>> for MatchResult {
  fn from(m: NodeMatch<L>) -> Self {
    let start = from_tuple(m.start_pos());
    let end = from_tuple(m.end_pos());
    Self { start, end }
  }
}

#[napi]
pub fn find_nodes(src: String, pattern: String) -> Vec<MatchResult> {
  let root = Tsx.ast_grep(src);
  let pattern = Pattern::new(&pattern, Tsx);
  root
    .root()
    .find_all(pattern)
    .map(MatchResult::from)
    .collect()
}
