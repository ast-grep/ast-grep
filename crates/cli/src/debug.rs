use crate::lang::SgLang;
use ast_grep_core::Pattern;
use ast_grep_language::Language;
use clap::ValueEnum;
use tree_sitter as ts;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DebugFormat {
  Pattern,
  Ast,
  Cst,
  Sexp,
}
impl DebugFormat {
  pub fn debug_query(&self, pattern: &str, lang: SgLang) {
    match self {
      DebugFormat::Pattern => {
        let pattern = Pattern::try_new(pattern, lang).expect("pattern must be validated in run");
        println!("Debug Pattern:\n{:?}", pattern);
      }
      DebugFormat::Sexp => {
        let root = lang.ast_grep(pattern);
        println!("Debug Sexp:\n{}", root.root().to_sexp());
      }
      DebugFormat::Ast => {
        let root = lang.ast_grep(pattern);
        let dumped = dump_node(root.root().get_ts_node());
        println!("Debug AST:\n{}", dumped.ast());
      }
      DebugFormat::Cst => {
        let root = lang.ast_grep(pattern);
        let dumped = dump_node(root.root().get_ts_node());
        println!("Debug CST:\n{}", dumped.cst());
      }
    }
  }
}

pub struct DumpNode {
  field: Option<String>,
  kind: String,
  start: Pos,
  end: Pos,
  is_named: bool,
  children: Vec<DumpNode>,
}

// TODO: add colorized output
use std::fmt::{Result as FmtResult, Write};
impl DumpNode {
  pub fn ast(&self) -> String {
    let mut result = String::new();
    self
      .helper(&mut result, true, 0)
      .expect("should write string");
    result
  }

  pub fn cst(&self) -> String {
    let mut result = String::new();
    self
      .helper(&mut result, false, 0)
      .expect("should write string");
    result
  }

  fn helper(&self, result: &mut String, named_only: bool, depth: usize) -> FmtResult {
    let indent = "  ".repeat(depth);
    if named_only && !self.is_named {
      return Ok(());
    }
    write!(result, "{indent}")?;
    if let Some(field) = &self.field {
      write!(result, "{field}: ")?;
    }
    write!(result, "{}", self.kind)?;
    writeln!(result, " ({:?})-({:?})", self.start, self.end)?;
    for child in &self.children {
      child.helper(result, named_only, depth + 1)?;
    }
    Ok(())
  }
}

pub struct Pos {
  row: u32,
  column: u32,
}

impl std::fmt::Debug for Pos {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{},{}", self.row, self.column)
  }
}

impl From<ts::Point> for Pos {
  #[inline]
  fn from(pt: ts::Point) -> Self {
    Pos {
      row: pt.row(),
      column: pt.column(),
    }
  }
}

fn dump_node(node: ts::Node) -> DumpNode {
  let mut cursor = node.walk();
  let mut nodes = vec![];
  dump_one_node(&mut cursor, &mut nodes);
  nodes.pop().expect("should have at least one node")
}

fn dump_one_node(cursor: &mut ts::TreeCursor, target: &mut Vec<DumpNode>) {
  let node = cursor.node();
  let kind = if node.is_missing() {
    format!("MISSING {}", node.kind())
  } else {
    node.kind().to_string()
  };
  let start = node.start_position().into();
  let end = node.end_position().into();
  let field = cursor.field_name().map(|c| c.to_string());
  let mut children = vec![];
  if cursor.goto_first_child() {
    dump_nodes(cursor, &mut children);
    cursor.goto_parent();
  }
  target.push(DumpNode {
    field,
    kind,
    start,
    end,
    children,
    is_named: node.is_named(),
  })
}

fn dump_nodes(cursor: &mut ts::TreeCursor, target: &mut Vec<DumpNode>) {
  loop {
    dump_one_node(cursor, target);
    if !cursor.goto_next_sibling() {
      break;
    }
  }
}
