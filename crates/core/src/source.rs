//! This module defines the `Doc` and `Content` traits to abstract away source code encoding issues.
//!
//! ast-grep supports three kinds of encoding: utf-8 for CLI, utf-16 for nodeJS napi and `Vec<char>` for wasm.
//! Different encoding will produce different tree-sitter Node's range and position.
//!
//! The `Content` trait is defined to abstract different encoding.
//! It is used as associated type bound `Source` in the `Doc` trait.
//! Its associated type `Underlying`  represents the underlying type of the content, e.g. `Vec<u8>`, `Vec<u16>`.
//!
//! `Doc` is a trait that defines a document that can be parsed by Tree-sitter.
//! It has a `Source` associated type bounded by `Content` that represents the source code of the document,
//! and a `Lang` associated type that represents the language of the document.

use crate::language::Language;
use std::borrow::Cow;
use std::ops::Range;
use thiserror::Error;
use tree_sitter::{
  InputEdit, Language as TsLang, LanguageError, Node, Parser, ParserError, Point, Tree,
};

#[inline]
fn parse_lang(
  parse_fn: impl Fn(&mut Parser) -> Result<Option<Tree>, ParserError>,
  ts_lang: TsLang,
) -> Result<Tree, TSParseError> {
  let mut parser = Parser::new()?;
  parser.set_language(&ts_lang)?;
  if let Some(tree) = parse_fn(&mut parser)? {
    Ok(tree)
  } else {
    Err(TSParseError::TreeUnavailable)
  }
}

// https://github.com/tree-sitter/tree-sitter/blob/e4e5ffe517ca2c668689b24cb17c51b8c6db0790/cli/src/parse.rs
#[derive(Debug)]
pub struct Edit<S: Content> {
  pub position: usize,
  pub deleted_length: usize,
  pub inserted_text: Vec<S::Underlying>,
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

pub fn perform_edit<S: Content>(tree: &mut Tree, input: &mut S, edit: &Edit<S>) -> InputEdit {
  let edit = input.accept_edit(edit);
  tree.edit(&edit);
  edit
}

/// Represents tree-sitter related error
#[derive(Debug, Error)]
pub enum TSParseError {
  #[error("web-tree-sitter parser is not available")]
  Parse(#[from] ParserError),
  #[error("incompatible `Language` is assigned to a `Parser`.")]
  Language(#[from] LanguageError),
  /// A general error when tree sitter fails to parse in time. It can be caused by
  /// the following reasons but tree-sitter does not provide error detail.
  /// * The timeout set with [Parser::set_timeout_micros] expired
  /// * The cancellation flag set with [Parser::set_cancellation_flag] was flipped
  /// * The parser has not yet had a language assigned with [Parser::set_language]
  #[error("general error when tree-sitter fails to parse.")]
  TreeUnavailable,
}

pub trait Doc: Clone {
  type Source: Content;
  type Lang: Language;
  fn get_lang(&self) -> &Self::Lang;
  fn get_source(&self) -> &Self::Source;
  fn get_source_mut(&mut self) -> &mut Self::Source;
  fn parse(&self, old_tree: Option<&Tree>) -> Result<Tree, TSParseError> {
    let source = self.get_source();
    let lang = self.get_lang().get_ts_language();
    parse_lang(|p| source.parse_tree_sitter(p, old_tree), lang)
  }
  fn clone_with_lang(&self, lang: Self::Lang) -> Self;
  /// TODO: are we paying too much to support str as Pattern/Replacer??
  /// this method converts string to Doc, so that we can support using
  /// string as replacer/searcher. Natively.
  fn from_str(src: &str, lang: Self::Lang) -> Self;
}

#[derive(Clone)]
pub struct StrDoc<L: Language> {
  pub src: String,
  pub lang: L,
}

impl<L: Language> StrDoc<L> {
  pub fn new(src: &str, lang: L) -> Self {
    Self {
      src: src.into(),
      lang,
    }
  }
}

impl<L: Language> Doc for StrDoc<L> {
  type Source = String;
  type Lang = L;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.src
  }
  fn get_source_mut(&mut self) -> &mut Self::Source {
    &mut self.src
  }
  fn from_str(src: &str, lang: L) -> Self {
    Self::new(src, lang)
  }
  fn clone_with_lang(&self, lang: Self::Lang) -> Self {
    Self::new(&self.src, lang)
  }
}

pub trait Content: Sized {
  type Underlying: Clone + PartialEq;
  fn parse_tree_sitter(
    &self,
    parser: &mut Parser,
    tree: Option<&Tree>,
  ) -> Result<Option<Tree>, ParserError>;
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying];
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit;
  fn get_text<'a>(&'a self, node: &Node) -> Cow<'a, str>;
  /// Used for string replacement. We need this for
  /// indentation and deindentation.
  fn decode_str(src: &str) -> Cow<[Self::Underlying]>;
  /// Used for string replacement. We need this for
  /// transformation.
  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<str>;
  /// Get the character column at the given position
  fn get_char_column(&self, column: usize, offset: usize) -> usize;
}

impl Content for String {
  type Underlying = u8;
  fn parse_tree_sitter(
    &self,
    parser: &mut Parser,
    tree: Option<&Tree>,
  ) -> Result<Option<Tree>, ParserError> {
    parser.parse(self.as_bytes(), tree)
  }
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying] {
    &self.as_bytes()[range]
  }
  fn get_text<'a>(&'a self, node: &Node) -> Cow<'a, str> {
    node
      .utf8_text(self.as_bytes())
      .expect("invalid source text encoding")
  }
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let input = unsafe { self.as_mut_vec() };
    let start_position = position_for_offset(input, start_byte);
    let old_end_position = position_for_offset(input, old_end_byte);
    input.splice(start_byte..old_end_byte, edit.inserted_text.clone());
    let new_end_position = position_for_offset(input, new_end_byte);
    InputEdit::new(
      start_byte as u32,
      old_end_byte as u32,
      new_end_byte as u32,
      &start_position,
      &old_end_position,
      &new_end_position,
    )
  }
  fn decode_str(src: &str) -> Cow<[Self::Underlying]> {
    Cow::Borrowed(src.as_bytes())
  }
  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<str> {
    String::from_utf8_lossy(bytes)
  }

  /// This is an O(n) operation. We assume the col will not be a
  /// huge number in reality. This may be problematic for special
  /// files like compressed js
  fn get_char_column(&self, _col: usize, offset: usize) -> usize {
    let src = self.as_bytes();
    let mut col = 0;
    // TODO: is it possible to use SIMD here???
    for &b in src[..offset].iter().rev() {
      if b == b'\n' {
        break;
      }
      // https://en.wikipedia.org/wiki/UTF-8#Description
      if b & 0b1100_0000 != 0b1000_0000 {
        col += 1;
      }
    }
    col
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};

  fn parse(src: &str) -> Result<Tree, TSParseError> {
    parse_lang(|p| p.parse(src, None), Tsx.get_ts_language())
  }

  #[test]
  fn test_tree_sitter() -> Result<(), TSParseError> {
    let tree = parse("var a = 1234")?;
    let root_node = tree.root_node();
    assert_eq!(root_node.kind(), "program");
    assert_eq!(root_node.start_position().column(), 0);
    assert_eq!(root_node.end_position().column(), 12);
    assert_eq!(
      root_node.to_sexp(),
      "(program (variable_declaration (variable_declarator name: (identifier) value: (number))))"
    );
    Ok(())
  }

  #[test]
  fn test_object_literal() -> Result<(), TSParseError> {
    let tree = parse("{a: $X}")?;
    let root_node = tree.root_node();
    // wow this is not label. technically it is wrong but practically it is better LOL
    assert_eq!(root_node.to_sexp(), "(program (expression_statement (object (pair key: (property_identifier) value: (identifier)))))");
    Ok(())
  }

  #[test]
  fn test_string() -> Result<(), TSParseError> {
    let tree = parse("'$A'")?;
    let root_node = tree.root_node();
    assert_eq!(
      root_node.to_sexp(),
      "(program (expression_statement (string (string_fragment))))"
    );
    Ok(())
  }

  #[test]
  fn test_row_col() -> Result<(), TSParseError> {
    let tree = parse("😄")?;
    let root = tree.root_node();
    assert_eq!(root.start_position(), Point::new(0, 0));
    // NOTE: Point in tree-sitter is counted in bytes instead of char
    assert_eq!(root.end_position(), Point::new(0, 4));
    Ok(())
  }

  #[test]
  fn test_edit() -> Result<(), TSParseError> {
    let mut src = "a + b".to_string();
    let mut tree = parse(&src)?;
    let _ = perform_edit(
      &mut tree,
      &mut src,
      &Edit {
        position: 1,
        deleted_length: 0,
        inserted_text: " * b".into(),
      },
    );
    let tree2 = parse_lang(|p| p.parse(&src, Some(&tree)), Tsx.get_ts_language())?;
    assert_eq!(
      tree.root_node().to_sexp(),
      "(program (expression_statement (binary_expression left: (identifier) right: (identifier))))"
    );
    assert_eq!(tree2.root_node().to_sexp(), "(program (expression_statement (binary_expression left: (binary_expression left: (identifier) right: (identifier)) right: (identifier))))");
    Ok(())
  }
}
