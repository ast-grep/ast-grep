use thiserror::Error;
use tree_sitter::{InputEdit, LanguageError, Parser, ParserError, Point};
pub use tree_sitter::{Language, Tree};

/// Represents tree-sitter related error
#[derive(Debug, Error)]
pub enum TSParseError {
  #[error("web-tree-sitter parser is not available")]
  Parse(#[from] ParserError),
  #[error("incompatible `Language` is assigend to a `Parser`.")]
  Language(#[from] LanguageError),
  /// A general error when tree sitter fails to parse in time. It can be caused by
  /// the following reasons but tree-sitter does not provide error detail.
  /// * The timeout set with [Parser::set_timeout_micros] expired
  /// * The cancellation flag set with [Parser::set_cancellation_flag] was flipped
  /// * The parser has not yet had a language assigned with [Parser::set_language]
  #[error("general error when tree-sitter fails to parse.")]
  TreeUnavailable,
}

pub fn parse(
  source_code: &str,
  old_tree: Option<&Tree>,
  ts_lang: Language,
) -> Result<Tree, TSParseError> {
  let mut parser = Parser::new()?;
  parser.set_language(&ts_lang)?;
  if let Some(tree) = parser.parse(source_code, old_tree)? {
    Ok(tree)
  } else {
    Err(TSParseError::TreeUnavailable)
  }
}

// https://github.com/tree-sitter/tree-sitter/blob/e4e5ffe517ca2c668689b24cb17c51b8c6db0790/cli/src/parse.rs
#[derive(Debug)]
pub struct Edit {
  pub position: usize,
  pub deleted_length: usize,
  pub inserted_text: String,
}

fn position_for_offset(input: &[u8], offset: usize) -> Point {
  debug_assert!(offset <= input.len());
  let (mut row, mut col) = (0, 0);
  for c in &input[0..offset] {
    if *c as char == '\n' {
      row += 1;
      col = 0;
    } else {
      col += 1;
    }
  }
  Point::new(row, col)
}

pub fn perform_edit(tree: &mut Tree, input: &mut Vec<u8>, edit: &Edit) -> InputEdit {
  let start_byte = edit.position;
  let old_end_byte = edit.position + edit.deleted_length;
  let new_end_byte = edit.position + edit.inserted_text.len();
  let start_position = position_for_offset(input, start_byte);
  let old_end_position = position_for_offset(input, old_end_byte);
  input.splice(start_byte..old_end_byte, edit.inserted_text.bytes());
  let new_end_position = position_for_offset(input, new_end_byte);
  let edit = InputEdit::new(
    start_byte as u32,
    old_end_byte as u32,
    new_end_byte as u32,
    &start_position,
    &old_end_position,
    &new_end_position,
  );
  tree.edit(&edit);
  edit
}

#[cfg(test)]
mod test {
  use super::{parse as parse_lang, *};
  use crate::language::{Language, Tsx};

  fn parse(src: &str) -> Tree {
    parse_lang(src, None, Tsx.get_ts_language()).unwrap()
  }

  #[test]
  fn test_tree_sitter() {
    let tree = parse("var a = 1234");
    let root_node = tree.root_node();
    assert_eq!(root_node.kind(), "program");
    assert_eq!(root_node.start_position().column(), 0);
    assert_eq!(root_node.end_position().column(), 12);
    assert_eq!(
      root_node.to_sexp(),
      "(program (variable_declaration (variable_declarator name: (identifier) value: (number))))"
    );
  }

  #[test]
  fn test_object_literal() {
    let tree = parse("{a: $X}");
    let root_node = tree.root_node();
    // wow this is not label. technically it is wrong but practically it is better LOL
    assert_eq!(root_node.to_sexp(), "(program (expression_statement (object (pair key: (property_identifier) value: (identifier)))))");
  }

  #[test]
  fn test_string() {
    let tree = parse("'$A'");
    let root_node = tree.root_node();
    assert_eq!(
      root_node.to_sexp(),
      "(program (expression_statement (string (string_fragment))))"
    );
  }

  #[test]
  fn test_edit() {
    let mut src = "a + b".to_string();
    let mut tree = parse(&src);
    let edit = perform_edit(
      &mut tree,
      unsafe { src.as_mut_vec() },
      &Edit {
        position: 1,
        deleted_length: 0,
        inserted_text: " * b".into(),
      },
    );
    tree.edit(&edit);
    let tree2 = parse_lang(&src, Some(&tree), Tsx.get_ts_language()).unwrap();
    assert_eq!(
      tree.root_node().to_sexp(),
      "(program (expression_statement (binary_expression left: (identifier) right: (identifier))))"
    );
    assert_eq!(tree2.root_node().to_sexp(), "(program (expression_statement (binary_expression left: (binary_expression left: (binary_expression left: (identifier) right: (identifier)) right: (identifier)) right: (identifier))))");
  }
}
