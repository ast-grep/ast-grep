pub mod traversal;

use crate::node::Root;
use crate::replacer::Replacer;
use crate::source::{Content, Doc, Edit, SgNode};
use crate::{node::KindId, Language, Position};
use crate::{AstGrep, Matcher};
use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZero;
use thiserror::Error;
pub use traversal::{TsPre, Visitor};
pub use tree_sitter::Language as TSLanguage;
use tree_sitter::{InputEdit, LanguageError, Node, Parser, Point, Tree};
pub use tree_sitter::{Point as TSPoint, Range as TSRange};

/// Represents tree-sitter related error
#[derive(Debug, Error)]
pub enum TSParseError {
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

#[inline]
fn parse_lang(
  parse_fn: impl Fn(&mut Parser) -> Option<Tree>,
  ts_lang: TSLanguage,
) -> Result<Tree, TSParseError> {
  let mut parser = Parser::new();
  parser.set_language(&ts_lang)?;
  if let Some(tree) = parse_fn(&mut parser) {
    Ok(tree)
  } else {
    Err(TSParseError::TreeUnavailable)
  }
}

#[derive(Clone)]
pub struct StrDoc<L: LanguageExt> {
  pub src: String,
  pub lang: L,
  pub tree: Tree,
}

impl<L: LanguageExt> StrDoc<L> {
  pub fn try_new(src: &str, lang: L) -> Result<Self, String> {
    let src = src.to_string();
    let ts_lang = lang.get_ts_language();
    let tree = parse_lang(|p| p.parse(src.as_bytes(), None), ts_lang).map_err(|e| e.to_string())?;
    Ok(Self { src, lang, tree })
  }
  pub fn new(src: &str, lang: L) -> Self {
    Self::try_new(src, lang).expect("Parser tree error")
  }
  fn parse(&self, old_tree: Option<&Tree>) -> Result<Tree, TSParseError> {
    let source = self.get_source();
    let lang = self.get_lang().get_ts_language();
    parse_lang(|p| p.parse(source.as_bytes(), old_tree), lang)
  }
}

impl<L: LanguageExt> Doc for StrDoc<L> {
  type Source = String;
  type Lang = L;
  type Node<'r> = Node<'r>;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.src
  }
  fn do_edit(&mut self, edit: &Edit<Self::Source>) -> Result<(), String> {
    let source = &mut self.src;
    perform_edit(&mut self.tree, source, edit);
    self.tree = self.parse(Some(&self.tree)).map_err(|e| e.to_string())?;
    Ok(())
  }
  fn root_node(&self) -> Node<'_> {
    self.tree.root_node()
  }
  fn get_node_text<'a>(&'a self, node: &Self::Node<'a>) -> Cow<'a, str> {
    Cow::Borrowed(
      node
        .utf8_text(self.src.as_bytes())
        .expect("invalid source text encoding"),
    )
  }
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
    Node::parent(self)
  }
  fn ancestors(&self, root: Self) -> impl Iterator<Item = Self> {
    let mut ancestor = Some(root);
    let self_id = self.id();
    std::iter::from_fn(move || {
      let inner = ancestor.take()?;
      if inner.id() == self_id {
        return None;
      }
      ancestor = inner.child_with_descendant(*self);
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
    Node::child(self, nth)
  }
  fn children(&self) -> impl ExactSizeIterator<Item = Self> {
    let mut cursor = self.walk();
    cursor.goto_first_child();
    NodeWalker {
      cursor,
      count: self.child_count(),
    }
  }
  fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
    Node::child_by_field_id(self, field_id)
  }
  fn next(&self) -> Option<Self> {
    self.next_sibling()
  }
  fn prev(&self) -> Option<Self> {
    self.prev_sibling()
  }
  fn next_all(&self) -> impl Iterator<Item = Self> {
    // if root is none, use self as fallback to return a type-stable Iterator
    let node = self.parent().unwrap_or(*self);
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
    let node = self.parent().unwrap_or(*self);
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
    Node::is_named(self)
  }
  /// N.B. it is different from is_named && is_leaf
  /// if a node has no named children.
  fn is_named_leaf(&self) -> bool {
    self.named_child_count() == 0
  }
  fn is_leaf(&self) -> bool {
    self.child_count() == 0
  }
  fn kind(&self) -> Cow<'_, str> {
    Cow::Borrowed(Node::kind(self))
  }
  fn kind_id(&self) -> KindId {
    Node::kind_id(self)
  }
  fn node_id(&self) -> usize {
    self.id()
  }
  fn range(&self) -> std::ops::Range<usize> {
    self.start_byte()..self.end_byte()
  }
  fn start_pos(&self) -> Position {
    let pos = self.start_position();
    let byte = self.start_byte();
    Position::new(pos.row, pos.column, byte)
  }
  fn end_pos(&self) -> Position {
    let pos = self.end_position();
    let byte = self.end_byte();
    Position::new(pos.row, pos.column, byte)
  }
  // missing node is a tree-sitter specific concept
  fn is_missing(&self) -> bool {
    Node::is_missing(self)
  }
  fn is_error(&self) -> bool {
    Node::is_error(self)
  }

  fn field(&self, name: &str) -> Option<Self> {
    self.child_by_field_name(name)
  }
  fn field_children(&self, field_id: Option<u16>) -> impl Iterator<Item = Self> {
    let field_id = field_id.and_then(NonZero::new);
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

pub fn perform_edit<S: ContentExt>(tree: &mut Tree, input: &mut S, edit: &Edit<S>) -> InputEdit {
  let edit = input.accept_edit(edit);
  tree.edit(&edit);
  edit
}

/// tree-sitter specific language trait
pub trait LanguageExt: Language {
  /// Create an [`AstGrep`] instance for the language
  fn ast_grep<S: AsRef<str>>(&self, source: S) -> AstGrep<StrDoc<Self>> {
    AstGrep::new(source, self.clone())
  }

  /// tree sitter language to parse the source
  fn get_ts_language(&self) -> TSLanguage;

  fn injectable_languages(&self) -> Option<&'static [&'static str]> {
    None
  }

  /// get injected language regions in the root document. e.g. get JavaScripts in HTML
  /// it will return a list of tuples of (language, regions).
  /// The first item is the embedded region language, e.g. javascript
  /// The second item is a list of regions in tree_sitter.
  /// also see https://tree-sitter.github.io/tree-sitter/using-parsers#multi-language-documents
  fn extract_injections<L: LanguageExt>(
    &self,
    _root: crate::Node<StrDoc<L>>,
  ) -> HashMap<String, Vec<TSRange>> {
    HashMap::new()
  }
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

impl<L: LanguageExt> AstGrep<StrDoc<L>> {
  pub fn new<S: AsRef<str>>(src: S, lang: L) -> Self {
    Root::str(src.as_ref(), lang)
  }

  pub fn source(&self) -> &str {
    self.doc.get_source().as_str()
  }

  pub fn generate(self) -> String {
    self.doc.src
  }
}

pub trait ContentExt: Content {
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit;
}
impl ContentExt for String {
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let input = unsafe { self.as_mut_vec() };
    let start_position = position_for_offset(input, start_byte);
    let old_end_position = position_for_offset(input, old_end_byte);
    input.splice(start_byte..old_end_byte, edit.inserted_text.clone());
    let new_end_position = position_for_offset(input, new_end_byte);
    InputEdit {
      start_byte,
      old_end_byte,
      new_end_byte,
      start_position,
      old_end_position,
      new_end_position,
    }
  }
}

impl<L: LanguageExt> Root<StrDoc<L>> {
  pub fn str(src: &str, lang: L) -> Self {
    Self::try_new(src, lang).expect("should parse")
  }
  pub fn try_new(src: &str, lang: L) -> Result<Self, String> {
    let doc = StrDoc::try_new(src, lang)?;
    Ok(Self { doc })
  }
  pub fn get_text(&self) -> &str {
    &self.doc.src
  }

  pub fn get_injections<F: Fn(&str) -> Option<L>>(&self, get_lang: F) -> Vec<Self> {
    let root = self.root();
    let range = self.lang().extract_injections(root);
    let roots = range
      .into_iter()
      .filter_map(|(lang, ranges)| {
        let lang = get_lang(&lang)?;
        let source = self.doc.get_source();
        let mut parser = Parser::new();
        parser.set_included_ranges(&ranges).ok()?;
        parser.set_language(&lang.get_ts_language()).ok()?;
        let tree = parser.parse(source, None)?;
        Some(Self {
          doc: StrDoc {
            src: self.doc.src.clone(),
            lang,
            tree,
          },
        })
      })
      .collect();
    roots
  }
}

pub struct DisplayContext<'r> {
  /// content for the matched node
  pub matched: Cow<'r, str>,
  /// content before the matched node
  pub leading: &'r str,
  /// content after the matched node
  pub trailing: &'r str,
  /// zero-based start line of the context
  pub start_line: usize,
}

/// these methods are only for `StrDoc`
impl<'r, L: LanguageExt> crate::Node<'r, StrDoc<L>> {
  #[doc(hidden)]
  pub fn display_context(&self, before: usize, after: usize) -> DisplayContext<'r> {
    let source = self.root.doc.get_source().as_str();
    let bytes = source.as_bytes();
    let start = self.inner.start_byte();
    let end = self.inner.end_byte();
    let (mut leading, mut trailing) = (start, end);
    let mut lines_before = before + 1;
    while leading > 0 {
      if bytes[leading - 1] == b'\n' {
        lines_before -= 1;
        if lines_before == 0 {
          break;
        }
      }
      leading -= 1;
    }
    let mut lines_after = after + 1;
    // tree-sitter will append line ending to source so trailing can be out of bound
    trailing = trailing.min(bytes.len());
    while trailing < bytes.len() {
      if bytes[trailing] == b'\n' {
        lines_after -= 1;
        if lines_after == 0 {
          break;
        }
      }
      trailing += 1;
    }
    // lines_before means we matched all context, offset is `before` itself
    let offset = if lines_before == 0 {
      before
    } else {
      // otherwise, there are fewer than `before` line in src, compute the actual line
      before + 1 - lines_before
    };
    DisplayContext {
      matched: self.text(),
      leading: &source[leading..start],
      trailing: &source[end..trailing],
      start_line: self.start_pos().line() - offset,
    }
  }

  pub fn replace_all<M: Matcher, R: Replacer<StrDoc<L>>>(
    &self,
    matcher: M,
    replacer: R,
  ) -> Vec<Edit<String>> {
    // TODO: support nested matches like Some(Some(1)) with pattern Some($A)
    Visitor::new(&matcher)
      .reentrant(false)
      .visit(self.clone())
      .map(|matched| matched.make_edit(&matcher, &replacer))
      .collect()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use tree_sitter::Point;

  fn parse(src: &str) -> Result<Tree, TSParseError> {
    parse_lang(|p| p.parse(src, None), Tsx.get_ts_language())
  }

  #[test]
  fn test_tree_sitter() -> Result<(), TSParseError> {
    let tree = parse("var a = 1234")?;
    let root_node = tree.root_node();
    assert_eq!(root_node.kind(), "program");
    assert_eq!(root_node.start_position().column, 0);
    assert_eq!(root_node.end_position().column, 12);
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
