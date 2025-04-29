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

use crate::{language::Language, node::KindId, Position};
use std::borrow::Cow;
use std::ops::Range;

// https://github.com/tree-sitter/tree-sitter/blob/e4e5ffe517ca2c668689b24cb17c51b8c6db0790/cli/src/parse.rs
#[derive(Debug)]
pub struct Edit<S: Content> {
  pub position: usize,
  pub deleted_length: usize,
  pub inserted_text: Vec<S::Underlying>,
}

/// NOTE: Some method names are the same as tree-sitter's methods.
/// Fully Qualified Syntax may needed https://stackoverflow.com/a/44445976/2198656
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

pub trait Doc: Clone + 'static {
  type Source: Content;
  type Lang: Language;
  type Node<'r>: SgNode<'r>;
  fn get_lang(&self) -> &Self::Lang;
  fn get_source(&self) -> &Self::Source;
  fn do_edit(&mut self, edit: &Edit<Self::Source>) -> Result<(), String>;
  fn root_node(&self) -> Self::Node<'_>;
  fn get_node_text<'a>(&'a self, node: &Self::Node<'a>) -> Cow<'a, str>;
}

pub trait Content: Sized {
  type Underlying: Clone + PartialEq;
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying];
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
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying] {
    &self.as_bytes()[range]
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
