use crate::language::Language;
use crate::matcher::{FindAllNodes, Matcher, NodeMatch};
use crate::replacer::Replacer;
use crate::source::{perform_edit, Content, Edit as E, TSParseError};
use crate::traversal::{Pre, Visitor};
use crate::{Doc, StrDoc};

type Edit<D> = E<<D as Doc>::Source>;

use std::borrow::Cow;

/// Represents [`tree_sitter::Tree`] and owns source string
/// Note: Root is generic against [`Language`](crate::language::Language)
#[derive(Clone)]
pub struct Root<D: Doc> {
  pub(crate) inner: tree_sitter::Tree,
  pub(crate) doc: D,
}

impl<L: Language> Root<StrDoc<L>> {
  pub fn str(src: &str, lang: L) -> Self {
    Self::try_new(src, lang).expect("should parse")
  }
  pub fn get_text(&self) -> &str {
    &self.doc.src
  }
}

impl<D: Doc> Root<D> {
  pub fn try_new(src: &str, lang: D::Lang) -> Result<Self, TSParseError> {
    let doc = D::from_str(src, lang);
    let inner = doc.parse(None)?;
    Ok(Self { inner, doc })
  }

  pub fn new(src: &str, lang: D::Lang) -> Self {
    Self::try_new(src, lang).expect("should parse")
  }
  pub fn try_doc(doc: D) -> Result<Self, TSParseError> {
    let inner = doc.parse(None)?;
    Ok(Self { inner, doc })
  }

  pub fn doc(doc: D) -> Self {
    Self::try_doc(doc).expect("Parse doc error")
  }

  pub fn lang(&self) -> &D::Lang {
    self.doc.get_lang()
  }
  /// The root node represents the entire source
  pub fn root(&self) -> Node<D> {
    Node {
      inner: self.inner.root_node(),
      root: self,
    }
  }

  // extract non generic implementation to reduce code size
  pub fn do_edit(&mut self, edit: Edit<D>) -> Result<(), TSParseError> {
    let source = self.doc.get_source_mut();
    let input_edit = perform_edit(&mut self.inner, source, &edit);
    self.inner.edit(&input_edit);
    self.inner = self.doc.parse(Some(&self.inner))?;
    Ok(())
  }

  /// Adopt the tree_sitter as the descendant of the root and return the wrapped sg Node.
  /// It assumes `inner` is the under the root and will panic at dev build if wrong node is used.
  pub fn adopt<'r>(&'r self, inner: tree_sitter::Node<'r>) -> Node<'r, D> {
    debug_assert!(self.check_lineage(&inner));
    Node { inner, root: self }
  }

  fn check_lineage(&self, inner: &tree_sitter::Node<'_>) -> bool {
    let mut node = inner.clone();
    while let Some(n) = node.parent() {
      node = n;
    }
    node == self.inner.root_node()
  }

  /// P.S. I am your father.
  #[doc(hidden)]
  pub unsafe fn readopt<'a: 'b, 'b>(&'a self, node: &mut Node<'b, D>) {
    debug_assert!(self.check_lineage(&node.inner));
    node.root = self;
  }
}

/// 'r represents root lifetime
#[derive(Clone)]
pub struct Node<'r, D: Doc> {
  pub(crate) inner: tree_sitter::Node<'r>,
  pub(crate) root: &'r Root<D>,
}
pub type KindId = u16;

struct NodeWalker<'tree, D: Doc> {
  cursor: tree_sitter::TreeCursor<'tree>,
  root: &'tree Root<D>,
  count: usize,
}

impl<'tree, D: Doc> Iterator for NodeWalker<'tree, D> {
  type Item = Node<'tree, D>;
  fn next(&mut self) -> Option<Self::Item> {
    if self.count == 0 {
      return None;
    }
    let ret = Some(Node {
      inner: self.cursor.node(),
      root: self.root,
    });
    self.cursor.goto_next_sibling();
    self.count -= 1;
    ret
  }
}

impl<'tree, D: Doc> ExactSizeIterator for NodeWalker<'tree, D> {
  fn len(&self) -> usize {
    self.count
  }
}

/// APIs for Node inspection
impl<'r, D: Doc> Node<'r, D> {
  pub fn node_id(&self) -> usize {
    self.inner.id()
  }
  pub fn is_leaf(&self) -> bool {
    self.inner.child_count() == 0
  }
  pub fn is_named_leaf(&self) -> bool {
    self.inner.named_child_count() == 0
  }
  pub fn is_error(&self) -> bool {
    self.inner.is_error()
  }
  pub fn kind(&self) -> Cow<str> {
    self.inner.kind()
  }
  pub fn kind_id(&self) -> KindId {
    self.inner.kind_id()
  }

  pub fn is_named(&self) -> bool {
    self.inner.is_named()
  }

  /// the underlying tree-sitter Node
  pub fn get_ts_node(&self) -> tree_sitter::Node<'r> {
    self.inner.clone()
  }

  /// byte offsets of start and end.
  pub fn range(&self) -> std::ops::Range<usize> {
    (self.inner.start_byte() as usize)..(self.inner.end_byte() as usize)
  }

  /// Nodes' start position in terms of zero-based rows and columns.
  pub fn start_pos(&self) -> (usize, usize) {
    let pos = self.inner.start_position();
    (pos.row() as usize, pos.column() as usize)
  }

  /// Nodes' end position in terms of rows and columns.
  pub fn end_pos(&self) -> (usize, usize) {
    let pos = self.inner.end_position();
    (pos.row() as usize, pos.column() as usize)
  }

  pub fn text(&self) -> Cow<'r, str> {
    let source = self.root.doc.get_source();
    source.get_text(&self.inner)
  }

  /// Node's tree structure dumped in Lisp like S-experssion
  pub fn to_sexp(&self) -> Cow<'_, str> {
    self.inner.to_sexp()
  }

  pub fn lang(&self) -> &D::Lang {
    self.root.lang()
  }
}

// TODO: figure out how to do this
impl<'r, L: Language> Node<'r, StrDoc<L>> {
  #[doc(hidden)]
  pub fn display_context(&self, before: usize, after: usize) -> DisplayContext<'r> {
    let source = self.root.doc.get_source().as_str();
    let bytes = source.as_bytes();
    let start = self.inner.start_byte() as usize;
    let end = self.inner.end_byte() as usize;
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
      start_line: self.start_pos().0 - offset,
    }
  }

  pub fn root(&self) -> &Root<StrDoc<L>> {
    self.root
  }
}

/**
 * Corresponds to inside/has/precedes/follows
 */
impl<'r, D: Doc> Node<'r, D> {
  pub fn matches<M: Matcher<D::Lang>>(&self, m: M) -> bool {
    m.match_node(self.clone()).is_some()
  }

  pub fn inside<M: Matcher<D::Lang>>(&self, m: M) -> bool {
    self.ancestors().find_map(|n| m.match_node(n)).is_some()
  }

  pub fn has<M: Matcher<D::Lang>>(&self, m: M) -> bool {
    self.dfs().skip(1).find_map(|n| m.match_node(n)).is_some()
  }

  pub fn precedes<M: Matcher<D::Lang>>(&self, m: M) -> bool {
    self.next_all().find_map(|n| m.match_node(n)).is_some()
  }

  pub fn follows<M: Matcher<D::Lang>>(&self, m: M) -> bool {
    self.prev_all().find_map(|n| m.match_node(n)).is_some()
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

  pub fn children<'s>(&'s self) -> impl ExactSizeIterator<Item = Node<'r, D>> + 's {
    let mut cursor = self.inner.walk();
    cursor.goto_first_child();
    NodeWalker {
      cursor,
      root: self.root,
      count: self.inner.child_count() as usize,
    }
  }

  #[must_use]
  pub fn child(&self, nth: usize) -> Option<Self> {
    // TODO: support usize
    let inner = self.inner.child(nth as u32)?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn field(&self, name: &str) -> Option<Self> {
    let mut cursor = self.inner.walk();
    let inner = self
      .inner
      .children_by_field_name(name, &mut cursor)
      .next()?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  pub fn field_children(&self, name: &str) -> impl Iterator<Item = Node<'r, D>> {
    let field_id = self
      .root
      .lang()
      .get_ts_language()
      .field_id_for_name(name)
      .unwrap_or(0);
    let root = self.root;
    let mut cursor = self.inner.walk();
    cursor.goto_first_child();
    let mut done = false;
    std::iter::from_fn(move || {
      if done {
        return None;
      }
      while cursor.field_id() != Some(field_id) {
        if !cursor.goto_next_sibling() {
          return None;
        }
      }
      let inner = cursor.node();
      if !cursor.goto_next_sibling() {
        done = true;
      }
      Some(Node { inner, root })
    })
  }

  /// Returns all ancestors nodes of `self`.
  /// Note: each invocation of the returned iterator is O(n)
  /// Using cursor is overkill here because adjust cursor is too expensive.
  pub fn ancestors(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    let mut parent = self.inner.parent();
    std::iter::from_fn(move || {
      let inner = parent.clone()?;
      let ret = Some(Node {
        inner: inner.clone(),
        root: self.root,
      });
      parent = inner.parent();
      ret
    })
  }
  #[must_use]
  pub fn next(&self) -> Option<Self> {
    let inner = self.inner.next_sibling()?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  /// Returns all sibling nodes next to `self`.
  // NOTE: Need go to parent first, then move to current node by byte offset.
  // This is because tree_sitter cursor is scoped to the starting node.
  // See https://github.com/tree-sitter/tree-sitter/issues/567
  // TODO: remove this target_arch dependent code branch
  // TODO: we need to add goto_first_child_for_byte in web-tree-sitter
  // https://github.com/tree-sitter/tree-sitter/discussions/2288
  #[cfg(not(target_arch = "wasm32"))]
  pub fn next_all(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    // if root is none, use self as fallback to return a type-stable Iterator
    let node = self.parent().unwrap_or_else(|| self.clone());
    let mut cursor = node.inner.walk();
    cursor.goto_first_child_for_byte(self.inner.start_byte());
    std::iter::from_fn(move || {
      if cursor.goto_next_sibling() {
        Some(self.root.adopt(cursor.node()))
      } else {
        None
      }
    })
  }

  // TODO: remove this target_arch dependent code branch
  // TODO: we need to add goto_first_child_for_byte in web-tree-sitter
  // https://github.com/tree-sitter/tree-sitter/discussions/2288
  #[cfg(target_arch = "wasm32")]
  pub fn next_all(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    let mut node = self.clone();
    std::iter::from_fn(move || {
      node.next().map(|n| {
        node = n.clone();
        n
      })
    })
  }

  #[must_use]
  pub fn prev(&self) -> Option<Node<'r, D>> {
    let inner = self.inner.prev_sibling()?;
    Some(Node {
      inner,
      root: self.root,
    })
  }

  // TODO: use cursor to optimize clone.
  // investigate why tree_sitter cursor cannot goto next_sibling
  pub fn prev_all(&self) -> impl Iterator<Item = Node<'r, D>> + '_ {
    let mut node = self.clone();
    std::iter::from_fn(move || {
      node.prev().map(|n| {
        node = n.clone();
        n
      })
    })
  }

  pub fn dfs<'s>(&'s self) -> Pre<'r, D> {
    Pre::new(self)
  }

  #[must_use]
  pub fn find<M: Matcher<D::Lang>>(&self, pat: M) -> Option<NodeMatch<'r, D>> {
    pat.find_node(self.clone())
  }

  pub fn find_all<M: Matcher<D::Lang>>(&self, pat: M) -> impl Iterator<Item = NodeMatch<'r, D>> {
    FindAllNodes::new(pat, self.clone())
  }
}

/// Tree manipulation API
impl<'r, D: Doc> Node<'r, D> {
  pub fn replace<M: Matcher<D::Lang>, R: Replacer<D>>(
    &self,
    matcher: M,
    replacer: R,
  ) -> Option<Edit<D>> {
    let matched = matcher.find_node(self.clone())?;
    let edit = matched.make_edit(&matcher, &replacer);
    Some(edit)
  }

  pub fn replace_all<M: Matcher<D::Lang>, R: Replacer<D>>(
    &self,
    matcher: M,
    replacer: R,
  ) -> Vec<Edit<D>> {
    // TODO: support nested matches like Some(Some(1)) with pattern Some($A)
    Visitor::new(&matcher)
      .reentrant(false)
      .visit(self.clone())
      .map(|matched| matched.make_edit(&matcher, &replacer))
      .collect()
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
}
