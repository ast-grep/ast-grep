use crate::language::Language;
use crate::matcher::{FindAllNodes, Matcher, NodeMatch};
use crate::replacer::Replacer;
use crate::source::{Content, Source};
use crate::traversal::{Pre, Visitor};
use crate::ts_parser::{parse, perform_edit, Edit, TSParseError};
use crate::{Doc, StrDoc};

use std::borrow::Cow;

/// Represents [`tree_sitter::Tree`] and owns source string
/// Note: Root is generic against [`Language`](crate::language::Language)
#[derive(Clone)]
pub struct Root<D: Doc> {
  pub(crate) inner: tree_sitter::Tree,
  pub(crate) doc: D,
}

impl<L: Language> Root<StrDoc<L>> {
  pub fn try_new(src: &str, lang: L) -> Result<Self, TSParseError> {
    let inner = parse(src, None, lang.get_ts_language())?;
    Ok(Self {
      inner,
      doc: StrDoc::new(src, lang),
    })
  }

  /*
  pub fn customized<C: Content>(content: C, lang: L) -> Result<Self, TSParseError> {
    let inner = parse(&content, None, lang.get_ts_language())?;
    Ok(Self {
      inner,
      source: Source::Customized(Box::new(content)),
      lang,
    })
  }
  */

  pub fn new(src: &str, lang: L) -> Self {
    Self::try_new(src, lang).expect("should parse")
  }
}

impl<D: Doc> Root<D> {
  pub fn source(&self) -> &str {
    self.doc.get_source()
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
  pub fn do_edit(&mut self, edit: Edit) -> Result<(), TSParseError> {
    let input = unsafe { self.doc.as_mut() };
    let input_edit = perform_edit(&mut self.inner, input, &edit);
    self.inner.edit(&input_edit);
    self.inner = parse(
      self.source(),
      Some(&self.inner),
      self.lang().get_ts_language(),
    )?;
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
    self
      .inner
      .utf8_text(self.root.source().as_bytes())
      .expect("invalid source text encoding")
  }

  /// Node's tree structure dumped in Lisp like S-experssion
  pub fn to_sexp(&self) -> Cow<'_, str> {
    self.inner.to_sexp()
  }

  #[doc(hidden)]
  pub fn display_context(&self, context_lines: usize) -> DisplayContext<'r> {
    let bytes = self.root.source().as_bytes();
    let start = self.inner.start_byte() as usize;
    let end = self.inner.end_byte() as usize;
    let (mut leading, mut trailing) = (start, end);
    let mut lines_before = context_lines + 1;
    while leading > 0 {
      if bytes[leading - 1] == b'\n' {
        lines_before -= 1;
        if lines_before == 0 {
          break;
        }
      }
      leading -= 1;
    }
    // tree-sitter will append line ending to source so trailing can be out of bound
    trailing = trailing.min(bytes.len() - 1);
    let mut lines_after = context_lines + 1;
    while trailing < bytes.len() - 1 {
      if bytes[trailing] == b'\n' || bytes[trailing + 1] == b'\n' {
        lines_after -= 1;
        if lines_after == 0 {
          break;
        }
      }
      trailing += 1;
    }
    DisplayContext {
      matched: self.text(),
      leading: &self.root.source()[leading..start],
      trailing: &self.root.source()[end..=trailing],
      start_line: self.inner.start_position().row() as usize + 1,
    }
  }

  pub fn lang(&self) -> &D::Lang {
    self.root.lang()
  }
}

/**
 * Corresponds to inside/has/precedes/follows
 */
impl<'r, L: Language> Node<'r, StrDoc<L>> {
  pub fn matches<M: Matcher<L>>(&self, m: M) -> bool {
    m.match_node(self.clone()).is_some()
  }

  pub fn inside<M: Matcher<L>>(&self, m: M) -> bool {
    self.ancestors().find_map(|n| m.match_node(n)).is_some()
  }

  pub fn has<M: Matcher<L>>(&self, m: M) -> bool {
    self.dfs().skip(1).find_map(|n| m.match_node(n)).is_some()
  }

  pub fn precedes<M: Matcher<L>>(&self, m: M) -> bool {
    self.next_all().find_map(|n| m.match_node(n)).is_some()
  }

  pub fn follows<M: Matcher<L>>(&self, m: M) -> bool {
    self.prev_all().find_map(|n| m.match_node(n)).is_some()
  }
}

#[doc(hidden)]
pub struct DisplayContext<'r> {
  /// content for the matched node
  pub matched: Cow<'r, str>,
  /// content before the matched node
  pub leading: &'r str,
  /// content after the matched node
  pub trailing: &'r str,
  /// start line of the matched node
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
}

impl<'r, L: Language> Node<'r, StrDoc<L>> {
  #[must_use]
  pub fn find<M: Matcher<L>>(&self, pat: M) -> Option<NodeMatch<'r, StrDoc<L>>> {
    pat.find_node(self.clone())
  }

  pub fn find_all<M: Matcher<L>>(&self, pat: M) -> impl Iterator<Item = NodeMatch<'r, StrDoc<L>>> {
    FindAllNodes::new(pat, self.clone())
  }
}

/// Tree manipulation API
impl<'r, L: Language> Node<'r, StrDoc<L>> {
  fn make_edit<M, R>(&self, matched: NodeMatch<StrDoc<L>>, matcher: &M, replacer: &R) -> Edit
  where
    M: Matcher<L>,
    R: Replacer<L>,
  {
    let lang = self.root.lang().clone();
    let env = matched.get_env();
    let range = matched.range();
    let position = range.start;
    let deleted_length = matcher
      .get_match_len(matched.get_node().clone())
      .unwrap_or_else(|| range.len());
    let inserted_text = replacer.generate_replacement(env, lang);
    Edit {
      position,
      deleted_length,
      inserted_text,
    }
  }

  pub fn replace<M: Matcher<L>, R: Replacer<L>>(&self, matcher: M, replacer: R) -> Option<Edit> {
    let matched = matcher.find_node(self.clone())?;
    let edit = self.make_edit(matched, &matcher, &replacer);
    Some(edit)
  }

  pub fn replace_all<M: Matcher<L>, R: Replacer<L>>(&self, matcher: M, replacer: R) -> Vec<Edit> {
    // TODO: support nested matches like Some(Some(1)) with pattern Some($A)
    Visitor::new(&matcher)
      .reentrant(false)
      .visit(self.clone())
      .map(|matched| self.make_edit(matched, &matcher, &replacer))
      .collect()
  }

  pub fn after(&self) -> Edit {
    todo!()
  }
  pub fn before(&self) -> Edit {
    todo!()
  }
  pub fn append(&self) -> Edit {
    todo!()
  }
  pub fn prepend(&self) -> Edit {
    todo!()
  }

  /// Empty children. Remove all child node
  pub fn empty(&self) -> Option<Edit> {
    let mut children = self.children().peekable();
    let start = children.peek()?.range().start;
    let end = children.last()?.range().end;
    Some(Edit {
      position: start,
      deleted_length: end - start,
      inserted_text: String::new(),
    })
  }

  /// Remove the node itself
  pub fn remove(&self) -> Edit {
    let range = self.range();
    Edit {
      position: range.start,
      deleted_length: range.end - range.start,
      inserted_text: String::new(),
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

  #[test]
  fn test_display_context() {
    // display context should not panic
    let s = "i()";
    assert_eq!(s.len(), 3);
    let root = Tsx.ast_grep(s);
    let node = root.root();
    assert_eq!(node.display_context(0).trailing.len(), 0);
  }

  #[test]
  fn test_replace_all_nested() {
    let root = Tsx.ast_grep("Some(Some(1))");
    let node = root.root();
    let edits = node.replace_all("Some($A)", "$A");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].inserted_text, "Some(1)");
  }

  #[test]
  fn test_replace_all_multiple_sorted() {
    let root = Tsx.ast_grep("Some(Some(1)); Some(2)");
    let node = root.root();
    let edits = node.replace_all("Some($A)", "$A");
    // edits must be sorted by position
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].inserted_text, "Some(1)");
    assert_eq!(edits[1].inserted_text, "2");
  }
}
