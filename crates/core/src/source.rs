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
  fn children(&self) -> impl ExactSizeIterator<Item = Self>;
  fn kind(&self) -> Cow<'_, str>;
  fn kind_id(&self) -> KindId;
  fn node_id(&self) -> usize;
  fn range(&self) -> std::ops::Range<usize>;
  fn start_pos(&self) -> Position;
  fn end_pos(&self) -> Position;

  // default implentation
  fn ancestors(&self, _root: Self) -> impl Iterator<Item = Self> {
    let mut ancestors = vec![];
    let mut current = self.clone();
    while let Some(parent) = current.parent() {
      ancestors.push(parent.clone());
      current = parent;
    }
    ancestors.reverse();
    ancestors.into_iter()
  }
  fn dfs(&self) -> impl Iterator<Item = Self> {
    let mut stack = vec![self.clone()];
    std::iter::from_fn(move || {
      if let Some(node) = stack.pop() {
        let children: Vec<_> = node.children().collect();
        stack.extend(children.into_iter().rev());
        Some(node)
      } else {
        None
      }
    })
  }
  fn child(&self, nth: usize) -> Option<Self> {
    self.children().nth(nth)
  }
  fn next(&self) -> Option<Self> {
    let parent = self.parent()?;
    let mut children = parent.children();
    while let Some(child) = children.next() {
      if child.node_id() == self.node_id() {
        return children.next();
      }
    }
    None
  }
  fn prev(&self) -> Option<Self> {
    let parent = self.parent()?;
    let children = parent.children();
    let mut prev = None;
    for child in children {
      if child.node_id() == self.node_id() {
        return prev;
      }
      prev = Some(child);
    }
    None
  }
  fn next_all(&self) -> impl Iterator<Item = Self> {
    let mut next = self.next();
    std::iter::from_fn(move || {
      let n = next.clone()?;
      next = n.next();
      Some(n)
    })
  }
  fn prev_all(&self) -> impl Iterator<Item = Self> {
    let mut prev = self.prev();
    std::iter::from_fn(move || {
      let n = prev.clone()?;
      prev = n.prev();
      Some(n)
    })
  }
  fn is_named(&self) -> bool {
    true
  }
  /// N.B. it is different from is_named && is_leaf
  /// if a node has no named children.
  fn is_named_leaf(&self) -> bool {
    self.is_leaf()
  }
  fn is_leaf(&self) -> bool {
    self.children().count() == 0
  }

  // missing node is a tree-sitter specific concept
  fn is_missing(&self) -> bool {
    false
  }
  fn is_error(&self) -> bool {
    false
  }

  fn field(&self, name: &str) -> Option<Self>;
  fn field_children(&self, field_id: Option<u16>) -> impl Iterator<Item = Self>;
  fn child_by_field_id(&self, field_id: u16) -> Option<Self>;
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
  fn decode_str(src: &str) -> Cow<'_, [Self::Underlying]>;
  /// Used for string replacement. We need this for
  /// transformation.
  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<'_, str>;
  /// Get the character column at the given position
  fn get_char_column(&self, column: usize, offset: usize) -> usize;
}

impl Content for String {
  type Underlying = u8;
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying] {
    &self.as_bytes()[range]
  }
  fn decode_str(src: &str) -> Cow<'_, [Self::Underlying]> {
    Cow::Borrowed(src.as_bytes())
  }
  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<'_, str> {
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
