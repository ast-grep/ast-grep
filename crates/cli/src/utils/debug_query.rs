use crate::lang::SgLang;
use ansi_term::Style;
use ast_grep_core::{language::TSLanguage, matcher::PatternNode, meta_var::MetaVariable, Pattern};
use ast_grep_language::Language;
use clap::ValueEnum;
use tree_sitter as ts;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DebugFormat {
  /// Print the query parsed in Pattern format
  Pattern,
  /// Print the query in tree-sitter AST format, only named nodes are shown
  Ast,
  /// Print the query in tree-sitter CST format, both named and unnamed nodes are shown
  Cst,
  /// Print the query in S-expression format
  Sexp,
}
impl DebugFormat {
  pub fn debug_pattern(&self, pattern: &Pattern<SgLang>, lang: SgLang, colored: bool) {
    match self {
      DebugFormat::Pattern => {
        let lang = lang.get_ts_language();
        let mut ret = String::new();
        let fmt = DumpFmt::named(colored);
        if dump_pattern(&pattern.node, &lang, &fmt, 0, &mut ret).is_ok() {
          eprintln!("Debug Pattern:\n{}", ret);
        } else {
          eprintln!("unexpected error in writing pattern string");
        }
      }
      DebugFormat::Sexp | DebugFormat::Ast | DebugFormat::Cst => {
        debug_assert!(false, "debug_pattern can only be called with pattern")
      }
    }
  }

  pub fn debug_tree(&self, src: &str, lang: SgLang, colored: bool) {
    let root = lang.ast_grep(src);
    match self {
      DebugFormat::Pattern => {
        debug_assert!(false, "debug_tree cannot be called with Pattern")
      }
      DebugFormat::Sexp => {
        eprintln!("Debug Sexp:\n{}", root.root().to_sexp());
      }
      DebugFormat::Ast => {
        let dumped = dump_node(root.root().get_ts_node());
        eprintln!("Debug AST:\n{}", dumped.ast(colored));
      }
      DebugFormat::Cst => {
        let dumped = dump_node(root.root().get_ts_node());
        eprintln!("Debug CST:\n{}", dumped.cst(colored));
      }
    }
  }
}

fn dump_pattern(
  pattern: &PatternNode,
  lang: &TSLanguage,
  style: &DumpFmt,
  indent: usize,
  ret: &mut String,
) -> FmtResult {
  write!(ret, "{}", "  ".repeat(indent))?;
  match pattern {
    PatternNode::MetaVar { meta_var } => {
      let meta_var = match meta_var {
        MetaVariable::Capture(name, _) => format!("${name}"),
        MetaVariable::MultiCapture(name) => format!("$$${name}"),
        MetaVariable::Multiple => "$$$".to_string(),
        MetaVariable::Dropped(_) => "$_".to_string(),
      };
      let meta_var = style.kind_style.paint(meta_var);
      writeln!(ret, "{} {meta_var}", style.field_style.paint("MetaVar"))?;
    }
    PatternNode::Terminal {
      text,
      kind_id,
      is_named,
    } => {
      let lang = lang.get_ts_language();
      if *is_named {
        let kind = lang.node_kind_for_id(*kind_id).unwrap();
        let kind = style.kind_style.paint(format!("{kind}"));
        writeln!(ret, "{kind} {text}")?;
      } else {
        writeln!(ret, "{text}")?;
      }
    }
    PatternNode::Internal { kind_id, children } => {
      let kind = lang.node_kind_for_id(*kind_id).unwrap();
      let kind = style.kind_style.paint(kind);
      writeln!(ret, "{kind}")?;
      for child in children {
        dump_pattern(child, lang, style, indent + 1, ret)?;
      }
    }
  }
  Ok(())
}

pub struct DumpNode {
  field: Option<String>,
  kind: String,
  start: Pos,
  end: Pos,
  is_named: bool,
  children: Vec<DumpNode>,
}

struct DumpFmt {
  kind_style: Style,
  field_style: Style,
  named_only: bool,
}

impl DumpFmt {
  fn named(colored: bool) -> Self {
    let style = Style::new();
    Self {
      kind_style: if colored { style.bold() } else { style },
      field_style: if colored { style.italic() } else { style },
      named_only: true,
    }
  }
  fn all(colored: bool) -> Self {
    let style = Style::new();
    Self {
      kind_style: if colored { style.bold() } else { style },
      field_style: if colored { style.italic() } else { style },
      named_only: false,
    }
  }
}

use std::fmt::{Result as FmtResult, Write};
impl DumpNode {
  pub fn ast(&self, colored: bool) -> String {
    let mut result = String::new();
    let fmt = DumpFmt::named(colored);
    self
      .helper(&mut result, &fmt, 0)
      .expect("should write string");
    result
  }

  pub fn cst(&self, colored: bool) -> String {
    let mut result = String::new();
    let fmt = DumpFmt::all(colored);
    self
      .helper(&mut result, &fmt, 0)
      .expect("should write string");
    result
  }

  fn helper(&self, result: &mut String, fmt: &DumpFmt, depth: usize) -> FmtResult {
    let indent = "  ".repeat(depth);
    if fmt.named_only && !self.is_named {
      return Ok(());
    }
    write!(result, "{indent}")?;
    if let Some(field) = &self.field {
      let field = fmt.field_style.paint(field);
      write!(result, "{}: ", field)?;
    }
    write!(result, "{}", fmt.kind_style.paint(&self.kind))?;
    writeln!(result, " ({:?})-({:?})", self.start, self.end)?;
    for child in &self.children {
      child.helper(result, fmt, depth + 1)?;
    }
    Ok(())
  }
}

pub struct Pos {
  row: usize,
  column: usize,
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
      row: pt.row() as usize,
      column: pt.column() as usize,
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

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_language::{TypeScript, C};
  const DUMPED: &str = r#"
program (0,0)-(0,11)
  variable_declaration (0,0)-(0,11)
    variable_declarator (0,4)-(0,11)
      name: identifier (0,4)-(0,5)
      value: number (0,8)-(0,11)"#;
  #[test]
  fn test_dump_node() {
    let lang = SgLang::Builtin(TypeScript.into());
    let root = lang.ast_grep("var a = 123");
    let dumped = dump_node(root.root().get_ts_node());
    assert_eq!(DUMPED.trim(), dumped.ast(false).trim());
  }

  const MISSING: &str = r#"
translation_unit (0,0)-(0,9)
  declaration (0,0)-(0,9)
    type: primitive_type (0,0)-(0,3)
    declarator: init_declarator (0,4)-(0,9)
      declarator: identifier (0,4)-(0,5)
      = (0,6)-(0,7)
      value: number_literal (0,8)-(0,9)
    MISSING ; (0,9)-(0,9)"#;
  #[test]
  fn test_missing_node() {
    let lang = SgLang::Builtin(C.into());
    let root = lang.ast_grep("int a = 1");
    let dumped = dump_node(root.root().get_ts_node());
    assert_eq!(MISSING.trim(), dumped.cst(false).trim());
  }
}
