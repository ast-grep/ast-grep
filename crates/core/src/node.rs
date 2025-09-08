use crate::matcher::{Matcher, MatcherExt, NodeMatch};
use crate::replacer::Replacer;
use crate::source::{Content, Edit as E, SgNode};
use crate::Doc;
use crate::Language;

type Edit<D> = E<<D as Doc>::Source>;

use std::borrow::Cow;

/// Represents a position in the source code.
/// The line and column are zero-based, character offsets.
/// It is different from tree-sitter's position which is zero-based `byte` offsets.
/// Note, accessing `column` is O(n) operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
  /// zero-based line offset. Text encoding does not matter.
  line: usize,
  /// zero-based BYTE offset instead of character offset
  byte_column: usize,
  /// byte offset of this position
  byte_offset: usize,
}

impl Position {
  pub fn new(line: usize, byte_column: usize, byte_offset: usize) -> Self {
    Self {
      line,
      byte_column,
      byte_offset,
    }
  }
  pub fn line(&self) -> usize {
    self.line
  }
  /// Returns the column in terms of characters.
  /// Note: node does not have to be a node of matching position.
  pub fn column<D: Doc>(&self, node: &Node<'_, D>) -> usize {
    let source = node.get_doc().get_source();
    source.get_char_column(self.byte_column, self.byte_offset)
  }
  pub fn byte_point(&self) -> (usize, usize) {
    (self.line, self.byte_column)
  }
}

/// Represents [`tree_sitter::Tree`] and owns source string
/// Note: Root is generic against [`Language`](crate::language::Language)
#[derive(Clone)]
pub struct Root<D: Doc> {
  pub(crate) doc: D,
}

impl<D: Doc> Root<D> {
  pub fn doc(doc: D) -> Self {
    Self { doc }
  }

  pub fn lang(&self) -> &D::Lang {
    self.doc.get_lang()
  }
  /// The root node represents the entire source
  pub fn root(&self) -> Node<'_, D> {
    Node {
      inner: self.doc.root_node(),
      root: self,
    }
  }

  // extract non generic implementation to reduce code size
  pub fn edit(&mut self, edit: Edit<D>) -> Result<&mut Self, String> {
    self.doc.do_edit(&edit)?;
    Ok(self)
  }

  pub fn replace<M: Matcher, R: Replacer<D>>(
    &mut self,
    pattern: M,
    replacer: R,
  ) -> Result<bool, String> {
    let root = self.root();
    if let Some(edit) = root.replace(pattern, replacer) {
      drop(root); // rust cannot auto drop root if D is not specified
      self.edit(edit)?;
      Ok(true)
    } else {
      Ok(false)
    }
  }

  /// Adopt the tree_sitter as the descendant of the root and return the wrapped sg Node.
  /// It assumes `inner` is the under the root and will panic at dev build if wrong node is used.
  pub fn adopt<'r>(&'r self, inner: D::Node<'r>) -> Node<'r, D> {
    debug_assert!(self.check_lineage(&inner));
    Node { inner, root: self }
  }

  fn check_lineage(&self, inner: &D::Node<'_>) -> bool {
    let mut node = inner.clone();
    while let Some(n) = node.parent() {
      node = n;
    }
    node.node_id() == self.doc.root_node().node_id()
  }

  /// P.S. I am your father.
  #[doc(hidden)]
  pub unsafe fn readopt<'a: 'b, 'b>(&'a self, node: &mut Node<'b, D>) {
    debug_assert!(self.check_lineage(&node.inner));
    node.root = self;
  }
}

// why we need one more content? https://github.com/ast-grep/ast-grep/issues/1951
/// 'r represents root lifetime
#[derive(Clone)]
pub struct Node<'r, D: Doc> {
  pub(crate) inner: D::Node<'r>,
  pub(crate) root: &'r Root<D>,
}
pub type KindId = u16;

/// APIs for Node inspection
impl<'r, D: Doc> Node<'r, D> {
  pub fn get_doc(&self) -> &'r D {
    &self.root.doc
  }
  pub fn node_id(&self) -> usize {
    self.inner.node_id()
  }
  pub fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }
  /// if has no named children.
  /// N.B. it is different from is_named && is_leaf
  // see https://github.com/ast-grep/ast-grep/issues/276
  pub fn is_named_leaf(&self) -> bool {
    self.inner.is_named_leaf()
  }
  pub fn is_error(&self) -> bool {
    self.inner.is_error()
  }
  pub fn kind(&self) -> Cow<'_, str> {
    self.inner.kind()
  }
  pub fn kind_id(&self) -> KindId {
    self.inner.kind_id()
  }

  pub fn is_named(&self) -> bool {
    self.inner.is_named()
  }
  pub fn is_missing(&self) -> bool {
    self.inner.is_missing()
  }

  /// byte offsets of start and end.
  pub fn range(&self) -> std::ops::Range<usize> {
    self.inner.range()
  }

  /// Nodes' start position in terms of zero-based rows and columns.
  pub fn start_pos(&self) -> Position {
    self.inner.start_pos()
  }

  /// Nodes' end position in terms of rows and columns.
  pub fn end_pos(&self) -> Position {
    self.inner.end_pos()
  }

  pub fn text(&self) -> Cow<'r, str> {
    self.root.doc.get_node_text(&self.inner)
  }

  pub fn lang(&self) -> &'r D::Lang {
    self.root.lang()
  }

  /// the underlying tree-sitter Node
  pub fn get_inner_node(&self) -> D::Node<'r> {
    self.inner.clone()
  }

  pub fn root(&self) -> &'r Root<D> {
    self.root
  }
}

/**
 * Corresponds to inside/has/precedes/follows
 */
impl<D: Doc> Node<'_, D> {
  pub fn matches<M: Matcher>(&self, m: M) -> bool {
    m.match_node(self.clone()).is_some()
  }

  pub fn inside<M: Matcher>(&self, m: M) -> bool {
    self.ancestors().find_map(|n| m.match_node(n)).is_some()
  }

  pub fn has<M: Matcher>(&self, m: M) -> bool {
    self.dfs().skip(1).find_map(|n| m.match_node(n)).is_some()
  }

  pub fn precedes<M: Matcher>(&self, m: M) -> bool {
    self.next_all().find_map(|n| m.match_node(n)).is_some()
  }

  pub fn follows<M: Matcher>(&self, m: M) -> bool {
    self.prev_all().find_map(|n| m.match_node(n)).is_some()
  }
}

/// tree traversal API
impl<'r, D: Doc> Node<'r, D> {
  #[must_use]
  pub fn parent(&self) -> Option<Self> {
    let inner = self.inner.parent()?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn children(&self) -> impl ExactSizeIterator<Item = Node<'r, D>> + '_ {
    self.inner.children().map(|inner| Node {
      inner,
      root: self.root,
    })
  }

  #[must_use]
  pub fn child(&self, nth: usize) -> Option<Self> {
    let inner = self.inner.child(nth)?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn field(&self, name: &str) -> Option<Self> {
    let inner = self.inner.field(name)?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
    let inner = self.inner.child_by_field_id(field_id)?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn field_children(&self, name: &str) -> impl Iterator<Item = Node<'r, D>> + '_ {
    let field_id = self.lang().field_to_id(name);
    self.inner.field_children(field_id).map(|inner| Node {
      inner,
      root: self.root,
    })
  }

  /// Returns all ancestors nodes of `self`.
  /// Using cursor is overkill here because adjust cursor is too expensive.
  pub fn ancestors(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    let root = self.root.doc.root_node();
    self.inner.ancestors(root).map(|inner| Node {
      inner,
      root: self.root,
    })
  }
  #[must_use]
  pub fn next(&self) -> Option<Self> {
    let inner = self.inner.next()?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  /// Returns all sibling nodes next to `self`.
  // NOTE: Need go to parent first, then move to current node by byte offset.
  // This is because tree_sitter cursor is scoped to the starting node.
  // See https://github.com/tree-sitter/tree-sitter/issues/567
  pub fn next_all(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    self.inner.next_all().map(|inner| Node {
      inner,
      root: self.root,
    })
  }

  #[must_use]
  pub fn prev(&self) -> Option<Node<'r, D>> {
    let inner = self.inner.prev()?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn prev_all(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    self.inner.prev_all().map(|inner| Node {
      inner,
      root: self.root,
    })
  }

  pub fn dfs<'s>(&'s self) -> impl Iterator<Item = Node<'r, D>> + 's {
    self.inner.dfs().map(|inner| Node {
      inner,
      root: self.root,
    })
  }

  #[must_use]
  pub fn find<M: Matcher>(&self, pat: M) -> Option<NodeMatch<'r, D>> {
    pat.find_node(self.clone())
  }

  pub fn find_all<'s, M: Matcher + 's>(
    &'s self,
    pat: M,
  ) -> impl Iterator<Item = NodeMatch<'r, D>> + 's {
    let kinds = pat.potential_kinds();
    self.dfs().filter_map(move |cand| {
      if let Some(k) = &kinds {
        if !k.contains(cand.kind_id().into()) {
          return None;
        }
      }
      pat.match_node(cand)
    })
  }
}

/// Tree manipulation API
impl<D: Doc> Node<'_, D> {
  pub fn replace<M: Matcher, R: Replacer<D>>(&self, matcher: M, replacer: R) -> Option<Edit<D>> {
    let matched = matcher.find_node(self.clone())?;
    let edit = matched.make_edit(&matcher, &replacer);
    Some(edit)
  }

  pub fn after(&self) -> Edit<D> {
    todo!()
  }
  pub fn before(&self) -> Edit<D> {
    todo!()
  }
  pub fn append(&self) -> Edit<D> {
    todo!()
  }
  pub fn prepend(&self) -> Edit<D> {
    todo!()
  }

  /// Empty children. Remove all child node
  pub fn empty(&self) -> Option<Edit<D>> {
    let mut children = self.children().peekable();
    let start = children.peek()?.range().start;
    let end = children.last()?.range().end;
    Some(Edit::<D> {
      position: start,
      deleted_length: end - start,
      inserted_text: Vec::new(),
    })
  }

  /// Remove the node itself
  pub fn remove(&self) -> Edit<D> {
    let range = self.range();
    Edit::<D> {
      position: range.start,
      deleted_length: range.end - range.start,
      inserted_text: Vec::new(),
    }
  }
}

#[cfg(test)]
mod test {
  use crate::language::{Language, Tsx};
  use crate::tree_sitter::LanguageExt;
  #[test]
  fn test_is_leaf() {
    let root = Tsx.ast_grep("let a = 123");
    let node = root.root();
    assert!(!node.is_leaf());
  }

  #[test]
  fn test_children() {
    let root = Tsx.ast_grep("let a = 123");
    let node = root.root();
    let children: Vec<_> = node.children().collect();
    assert_eq!(children.len(), 1);
    let texts: Vec<_> = children[0]
      .children()
      .map(|c| c.text().to_string())
      .collect();
    assert_eq!(texts, vec!["let", "a = 123"]);
  }
  #[test]
  fn test_empty() {
    let root = Tsx.ast_grep("let a = 123");
    let node = root.root();
    let edit = node.empty().unwrap();
    assert_eq!(edit.inserted_text.len(), 0);
    assert_eq!(edit.deleted_length, 11);
    assert_eq!(edit.position, 0);
  }

  #[test]
  fn test_field_children() {
    let root = Tsx.ast_grep("let a = 123");
    let node = root.root().find("let a = $A").unwrap();
    let children: Vec<_> = node.field_children("kind").collect();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].text(), "let");
  }

  const MULTI_LINE: &str = "
if (a) {
  test(1)
} else {
  x
}
";

  #[test]
  fn test_display_context() {
    // src, matcher, lead, trail
    let cases = [
      ["i()", "i()", "", ""],
      ["i()", "i", "", "()"],
      [MULTI_LINE, "test", "  ", "(1)"],
    ];
    // display context should not panic
    for [src, matcher, lead, trail] in cases {
      let root = Tsx.ast_grep(src);
      let node = root.root().find(matcher).expect("should match");
      let display = node.display_context(0, 0);
      assert_eq!(display.leading, lead);
      assert_eq!(display.trailing, trail);
    }
  }

  #[test]
  fn test_multi_line_context() {
    let cases = [
      ["i()", "i()", "", ""],
      [MULTI_LINE, "test", "if (a) {\n  ", "(1)\n} else {"],
    ];
    // display context should not panic
    for [src, matcher, lead, trail] in cases {
      let root = Tsx.ast_grep(src);
      let node = root.root().find(matcher).expect("should match");
      let display = node.display_context(1, 1);
      assert_eq!(display.leading, lead);
      assert_eq!(display.trailing, trail);
    }
  }

  #[test]
  fn test_replace_all_nested() {
    let root = Tsx.ast_grep("Some(Some(1))");
    let node = root.root();
    let edits = node.replace_all("Some($A)", "$A");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].inserted_text, "Some(1)".as_bytes());
  }

  #[test]
  fn test_replace_all_multiple_sorted() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2)");
    let node = root.root();
    let edits = node.replace_all("Some($A)", "$A");
    // edits must be sorted by position
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].inserted_text, "Some(1)".as_bytes());
    assert_eq!(edits[1].inserted_text, "2".as_bytes());
  }

  #[test]
  fn test_inside() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2)");
    let root = root.root();
    let node = root.find("Some(1)").expect("should exist");
    assert!(node.inside("Some($A)"));
  }
  #[test]
  fn test_has() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2)");
    let root = root.root();
    let node = root.find("Some($A)").expect("should exist");
    assert!(node.has("Some(1)"));
  }
  #[test]
  fn precedes() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2);");
    let root = root.root();
    let node = root.find("Some($A);").expect("should exist");
    assert!(node.precedes("Some(2);"));
  }
  #[test]
  fn follows() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2);");
    let root = root.root();
    let node = root.find("Some(2);").expect("should exist");
    assert!(node.follows("Some(Some(1));"));
  }

  #[test]
  fn test_field() {
    let root = Tsx.ast_grep("class A{}");
    let root = root.root();
    let node = root.find("class $C {}").expect("should exist");
    assert!(node.field("name").is_some());
    assert!(node.field("none").is_none());
  }
  #[test]
  fn test_child_by_field_id() {
    let root = Tsx.ast_grep("class A{}");
    let root = root.root();
    let node = root.find("class $C {}").expect("should exist");
    let id = Tsx.field_to_id("name").unwrap();
    assert!(node.child_by_field_id(id).is_some());
    assert!(node.child_by_field_id(id + 1).is_none());
  }

  #[test]
  fn test_remove() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2);");
    let root = root.root();
    let node = root.find("Some(2);").expect("should exist");
    let edit = node.remove();
    assert_eq!(edit.position, 15);
    assert_eq!(edit.deleted_length, 8);
  }

  #[test]
  fn test_ascii_pos() {
    let root = Tsx.ast_grep("a");
    let root = root.root();
    let node = root.find("$A").expect("should exist");
    assert_eq!(node.start_pos().line(), 0);
    assert_eq!(node.start_pos().column(&*node), 0);
    assert_eq!(node.end_pos().line(), 0);
    assert_eq!(node.end_pos().column(&*node), 1);
  }

  #[test]
  fn test_unicode_pos() {
    let root = Tsx.ast_grep("🦀");
    let root = root.root();
    let node = root.find("$A").expect("should exist");
    assert_eq!(node.start_pos().line(), 0);
    assert_eq!(node.start_pos().column(&*node), 0);
    assert_eq!(node.end_pos().line(), 0);
    assert_eq!(node.end_pos().column(&*node), 1);
    let root = Tsx.ast_grep("\n  🦀🦀");
    let root = root.root();
    let node = root.find("$A").expect("should exist");
    assert_eq!(node.start_pos().line(), 1);
    assert_eq!(node.start_pos().column(&*node), 2);
    assert_eq!(node.end_pos().line(), 1);
    assert_eq!(node.end_pos().column(&*node), 4);
  }
}
