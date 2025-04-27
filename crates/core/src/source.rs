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

use crate::traversal::TsPre;
use crate::{language::Language, node::KindId, Position};
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

pub trait SgNode<'r>: Clone {
  fn parent(&self) -> Option<Self>;
  fn ancestors(&self, root: Self) -> impl Iterator<Item = Self>;
  fn dfs(&self) -> impl Iterator<Item = Self>;
  fn child(&self, nth: usize) -> Option<Self>;
  fn children(&self) -> impl ExactSizeIterator<Item = Self>;
  fn child_by_field_id(&self, field_id: u16) -> Option<Self>;
  fn next(&self) -> Option<Self>;
  fn prev(&self) -> Option<Self>;
  fn next_all(&self) -> impl Iterator<Item = Self>;
  fn prev_all(&self) -> impl Iterator<Item = Self>;
  fn is_named(&self) -> bool;
  /// N.B. it is different from is_named && is_leaf
  /// if a node has no named children.
  fn is_named_leaf(&self) -> bool;
  fn is_leaf(&self) -> bool;
  fn kind(&self) -> Cow<str>;
  fn kind_id(&self) -> KindId;
  fn node_id(&self) -> usize;
  fn range(&self) -> std::ops::Range<usize>;
  fn start_pos(&self) -> Position;
  fn end_pos(&self) -> Position;
  // missing node is a tree-sitter specific concept
  fn is_missing(&self) -> bool;
  fn is_error(&self) -> bool;

  fn field(&self, name: &str) -> Option<Self>;
  fn field_children(&self, field_id: Option<u16>) -> impl Iterator<Item = Self>;
}

struct NodeWalker<'tree> {
  cursor: tree_sitter::TreeCursor<'tree>,
  count: usize,
}

impl<'tree> Iterator for NodeWalker<'tree> {
  type Item = Node<'tree>;
  fn next(&mut self) -> Option<Self::Item> {
    if self.count == 0 {
      return None;
    }
    let ret = Some(self.cursor.node());
    self.cursor.goto_next_sibling();
    self.count -= 1;
    ret
  }
}

impl ExactSizeIterator for NodeWalker<'_> {
  fn len(&self) -> usize {
    self.count
  }
}

impl<'r> SgNode<'r> for Node<'r> {
  fn parent(&self) -> Option<Self> {
    self.parent()
  }
  fn ancestors(&self, root: Self) -> impl Iterator<Item = Self> {
    let mut ancestor = Some(root);
    let self_id = self.id();
    std::iter::from_fn(move || {
      let inner = ancestor.take()?;
      if inner.id() == self_id {
        return None;
      }
      ancestor = inner.child_with_descendant(self.clone());
      Some(inner)
    })
    // We must iterate up the tree to preserve backwards compatibility
    .collect::<Vec<_>>()
    .into_iter()
    .rev()
  }
  fn dfs(&self) -> impl Iterator<Item = Self> {
    TsPre::new(self)
  }
  fn child(&self, nth: usize) -> Option<Self> {
    // TODO remove cast after migrating to tree-sitter
    self.child(nth as u32)
  }
  fn children(&self) -> impl ExactSizeIterator<Item = Self> {
    let cursor = self.walk();
    NodeWalker {
      cursor,
      count: self.child_count() as usize,
    }
  }
  fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
    self.child_by_field_id(field_id)
  }
  fn next(&self) -> Option<Self> {
    self.next_sibling()
  }
  fn prev(&self) -> Option<Self> {
    self.prev_sibling()
  }
  fn next_all(&self) -> impl Iterator<Item = Self> {
    // if root is none, use self as fallback to return a type-stable Iterator
    let node = self.parent().unwrap_or_else(|| self.clone());
    let mut cursor = node.walk();
    cursor.goto_first_child_for_byte(self.start_byte());
    std::iter::from_fn(move || {
      if cursor.goto_next_sibling() {
        Some(cursor.node())
      } else {
        None
      }
    })
  }
  fn prev_all(&self) -> impl Iterator<Item = Self> {
    // if root is none, use self as fallback to return a type-stable Iterator
    let node = self.parent().unwrap_or_else(|| self.clone());
    let mut cursor = node.walk();
    cursor.goto_first_child_for_byte(self.start_byte());
    std::iter::from_fn(move || {
      if cursor.goto_previous_sibling() {
        Some(cursor.node())
      } else {
        None
      }
    })
  }
  fn is_named(&self) -> bool {
    self.is_named()
  }
  /// N.B. it is different from is_named && is_leaf
  /// if a node has no named children.
  fn is_named_leaf(&self) -> bool {
    self.named_child_count() == 0
  }
  fn is_leaf(&self) -> bool {
    self.child_count() == 0
  }
  fn kind(&self) -> Cow<str> {
    self.kind()
  }
  fn kind_id(&self) -> KindId {
    self.kind_id()
  }
  fn node_id(&self) -> usize {
    self.id()
  }
  fn range(&self) -> std::ops::Range<usize> {
    (self.start_byte() as usize)..(self.end_byte() as usize)
  }
  fn start_pos(&self) -> Position {
    let pos = self.start_position();
    let byte = self.start_byte() as usize;
    Position::new(pos.row() as usize, pos.column() as usize, byte)
  }
  fn end_pos(&self) -> Position {
    let pos = self.end_position();
    let byte = self.end_byte() as usize;
    Position::new(pos.row() as usize, pos.column() as usize, byte)
  }
  // missing node is a tree-sitter specific concept
  fn is_missing(&self) -> bool {
    self.is_missing()
  }
  fn is_error(&self) -> bool {
    self.is_error()
  }

  fn field(&self, name: &str) -> Option<Self> {
    self.child_by_field_name(name)
  }
  fn field_children(&self, field_id: Option<u16>) -> impl Iterator<Item = Self> {
    let mut cursor = self.walk();
    cursor.goto_first_child();
    // if field_id is not found, iteration is done
    let mut done = field_id.is_none();

    std::iter::from_fn(move || {
      if done {
        return None;
      }
      while cursor.field_id() != field_id {
        if !cursor.goto_next_sibling() {
          return None;
        }
      }
      let ret = cursor.node();
      if !cursor.goto_next_sibling() {
        done = true;
      }
      Some(ret)
    })
  }
}

pub trait Doc: Clone + 'static {
  type Source: Content;
  type Lang: Language;
  type Node<'r>: SgNode<'r>;
  fn get_lang(&self) -> &Self::Lang;
  fn get_source(&self) -> &Self::Source;
  fn do_edit(&mut self, edit: &Edit<Self::Source>);
  fn root_node(&self) -> Self::Node<'_>;
  fn get_node_text<'a>(&'a self, node: &Self::Node<'a>) -> Cow<'a, str>;
}

#[derive(Clone)]
pub struct StrDoc<L: Language> {
  pub src: String,
  pub lang: L,
  pub tree: Tree,
}

impl<L: Language> StrDoc<L> {
  pub fn new(src: &str, lang: L) -> Self {
    let src = src.to_string();
    let tree = parse_lang(|p| src.parse_tree_sitter(p, None), lang.get_ts_language());
    Self {
      src,
      lang,
      tree: tree.expect("TODO!"),
    }
  }
  pub fn from_str(src: &str, lang: L) -> Self {
    Self::new(src, lang)
  }
  fn parse(&self, old_tree: Option<&Tree>) -> Result<Tree, TSParseError> {
    let source = self.get_source();
    let lang = self.get_lang().get_ts_language();
    parse_lang(|p| source.parse_tree_sitter(p, old_tree), lang)
  }
}

impl<L: Language> Doc for StrDoc<L> {
  type Source = String;
  type Lang = L;
  type Node<'r> = Node<'r>;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.src
  }
  fn do_edit(&mut self, edit: &Edit<Self::Source>) {
    let source = &mut self.src;
    perform_edit(&mut self.tree, source, edit);
    self.tree = self.parse(Some(&self.tree)).expect("TODO!");
  }
  fn root_node(&self) -> Node<'_> {
    self.tree.root_node()
  }
  fn get_node_text<'a>(&'a self, node: &Self::Node<'a>) -> Cow<'a, str> {
    node
      .utf8_text(self.src.as_bytes())
      .expect("invalid source text encoding")
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
