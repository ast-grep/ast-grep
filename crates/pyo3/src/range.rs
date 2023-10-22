use ast_grep_core::{Doc, Node};
use pyo3::prelude::*;
use std::fmt::{self, Debug, Display, Formatter};

#[pyclass(frozen, get_all)]
#[derive(Clone)]
pub struct Pos {
  /// line number starting from 1
  line: u32,
  /// column number starting from 1
  column: u32,
  // TODO: this should be char offset
  /// byte offset of the position
  index: u32,
}

impl Display for Pos {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
    write!(f, "({},{})", self.line, self.column)
  }
}

impl Debug for Pos {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
    write!(
      f,
      "Pos(line={}, col={}, index={})",
      self.line, self.column, self.index
    )
  }
}

#[pymethods]
impl Pos {
  fn __repr__(&self) -> String {
    format!("{:?}", self)
  }
  fn __str__(&self) -> String {
    self.to_string()
  }
}

fn to_pos(pos: (usize, usize), offset: usize) -> Pos {
  Pos {
    line: pos.0 as u32,
    column: pos.1 as u32 / 2,
    index: offset as u32 / 2,
  }
}

#[pyclass(frozen, get_all)]
#[derive(Clone)]
pub struct Range {
  /// starting position of the range
  start: Pos,
  /// ending position of the range
  end: Pos,
}

impl Display for Range {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
    write!(f, "{}-{}", self.start, self.end)
  }
}

impl Debug for Range {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
    write!(f, "Range(start={:?}, end={:?})", self.start, self.end)
  }
}

#[pymethods]
impl Range {
  fn __repr__(&self) -> String {
    format!("{:?}", self)
  }

  fn __str__(&self) -> String {
    self.to_string()
  }
}

impl Range {
  pub fn from<D: Doc>(node: &Node<D>) -> Self {
    let byte_range = node.range();
    let start_pos = node.start_pos();
    let end_pos = node.end_pos();
    Range {
      start: to_pos(start_pos, byte_range.start),
      end: to_pos(end_pos, byte_range.end),
    }
  }
}
