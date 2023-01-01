//! # Traverse Node AST
//!
//! ast-grep supports common tree traversal algorithms, including
//! * Pre order traversal
//! * Post order traversal
//! * Level order traversal
//!
//! Note tree traversal can also be used with Matcher. A traversal with Matcher will
//! produce a [`NodeMatch`] sequence where all items satisfies the Matcher.
//!
//! It is also possible to specify the reentrancy of a traversal.
//! That is, we can control whether a matching node should be visited when it is nested within another match.
//! For example, suppose we want to find all usages of calling `foo` in the source `foo(foo())`.
//! The code has two matching calls and we can configure a traversal
//! to report only the inner one, only the outer one or both.
//!
//! Pre and Post order traversals in this module are implemented using tree-sitter's cursor API without extra heap allocation.
//! It is recommended to use traversal instead of tree recursion to avoid stack overflow and memory overhead.
//! Level order is also included for completeness and should be used sparingly.

use crate::language::Language;
use crate::matcher::Matcher;
use crate::{Node, Root};

use tree_sitter as ts;

use std::collections::VecDeque;
use std::iter::FusedIterator;
use std::marker::PhantomData;

pub struct Visitor<M, A = PreOrder> {
  /// Whether a node will match if it contains or is contained in another match.
  pub reentrant: bool,
  /// Whether visit named node only
  pub named_only: bool,
  /// optional matcher to filter nodes
  pub matcher: M,
  /// The algorithm to traverse the tree, can be pre/post/level order
  pub algorithm: PhantomData<A>,
}

impl<M> Visitor<M> {
  pub fn new<A>(matcher: M) -> Visitor<M, A> {
    Visitor {
      reentrant: true,
      named_only: false,
      matcher,
      algorithm: PhantomData,
    }
  }
  pub fn algorithm<A>(self) -> Visitor<M, A> {
    Visitor {
      reentrant: self.reentrant,
      named_only: self.named_only,
      matcher: self.matcher,
      algorithm: PhantomData,
    }
  }
}

impl<M, A> Visitor<M, A>
where
  A: Algorithm,
{
  pub fn visit<L: Language>(self, node: Node<L>) -> Visit<'_, L, A::Traversal<'_, L>, M>
  where
    M: Matcher<L>,
  {
    let traversal = A::traverse(node);
    Visit {
      reentrant: self.reentrant,
      named: self.named_only,
      matcher: self.matcher,
      traversal,
      lang: PhantomData,
    }
  }
}

pub struct Visit<'t, L, T, M> {
  reentrant: bool,
  named: bool,
  matcher: M,
  traversal: T,
  lang: PhantomData<&'t L>,
}
impl<'t, L, T, M> Visit<'t, L, T, M>
where
  L: Language + 't,
  T: Traversal<'t, L>,
  M: Matcher<L>,
{
  fn mark_match(&mut self, matched: bool) {
    if !self.reentrant {
      self.traversal.mark_last_node_matched(matched);
    }
  }
}

impl<'t, L, T, M> Iterator for Visit<'t, L, T, M>
where
  L: Language + 't,
  T: Traversal<'t, L>,
  M: Matcher<L>,
{
  type Item = Node<'t, L>;
  fn next(&mut self) -> Option<Self::Item> {
    while let Some(node) = self.traversal.next() {
      let pass_named = !self.named || node.is_named();
      if pass_named && node.matches(&self.matcher) {
        self.mark_match(true);
        return Some(node);
      } else {
        self.mark_match(false);
      }
    }
    None
  }
}

pub trait Algorithm {
  type Traversal<'t, L: 't + Language>: Traversal<'t, L>;
  fn traverse<L: Language>(node: Node<L>) -> Self::Traversal<'_, L>;
}

pub struct PreOrder;
impl Algorithm for PreOrder {
  type Traversal<'t, L: 't + Language> = Pre<'t, L>;
  fn traverse<L: Language>(node: Node<L>) -> Self::Traversal<'_, L> {
    Pre::new(&node)
  }
}
pub struct PostOrder;
impl Algorithm for PostOrder {
  type Traversal<'t, L: 't + Language> = Post<'t, L>;
  fn traverse<L: Language>(node: Node<L>) -> Self::Traversal<'_, L> {
    Post::new(&node)
  }
}

pub trait Traversal<'t, L: Language + 't>: Iterator<Item = Node<'t, L>> {
  fn mark_last_node_matched(&mut self, matched: bool);
}

/// Represents a pre-order traversal
pub struct Pre<'tree, L: Language> {
  cursor: ts::TreeCursor<'tree>,
  root: &'tree Root<L>,
  // record the starting node, if we return back to starting point
  // we should terminate the dfs.
  start_id: Option<usize>,
}

impl<'tree, L: Language> Pre<'tree, L> {
  pub fn new(node: &Node<'tree, L>) -> Self {
    Self {
      cursor: node.inner.walk(),
      root: node.root,
      start_id: Some(node.inner.id()),
    }
  }
}

/// Amortized time complexity is O(NlgN), depending on branching factor.
impl<'tree, L: Language> Iterator for Pre<'tree, L> {
  type Item = Node<'tree, L>;
  // 1. Yield the node itself
  // 2. Try visit the child node until no child available
  // 3. Try visit next sibling after going back to parent
  // 4. Repeat step 3 until returning to the starting node
  fn next(&mut self) -> Option<Self::Item> {
    // start_id will always be Some until the dfs terminates
    let start = self.start_id?;
    let cursor = &mut self.cursor;
    let inner = cursor.node(); // get current node
    let ret = Some(Node {
      inner,
      root: self.root,
    });
    // try going to children first
    if cursor.goto_first_child() {
      return ret;
    }
    // if no child available, go to ancestor nodes
    // until we get to the starting point
    while cursor.node().id() != start {
      // try visit sibling nodes
      if cursor.goto_next_sibling() {
        return ret;
      }
      // go back to parent node
      cursor.goto_parent();
    }
    // terminate traversal here
    self.start_id = None;
    ret
  }
}
impl<'tree, L: Language> FusedIterator for Pre<'tree, L> {}

impl<'t, L: Language> Traversal<'t, L> for Pre<'t, L> {
  #[inline]
  fn mark_last_node_matched(&mut self, matched: bool) {
    if matched {}
  }
}

/// Represents a post-order traversal
pub struct Post<'tree, L: Language> {
  cursor: ts::TreeCursor<'tree>,
  root: &'tree Root<L>,
  start_id: Option<usize>,
}

/// Amortized time complexity is O(NlgN), depending on branching factor.
impl<'tree, L: Language> Post<'tree, L> {
  pub fn new(node: &Node<'tree, L>) -> Self {
    let mut cursor = node.inner.walk();
    dive_down(&mut cursor);
    Self {
      cursor,
      root: node.root,
      start_id: Some(node.inner.id()),
    }
  }
}

fn dive_down(cursor: &mut ts::TreeCursor) {
  while cursor.goto_first_child() {
    continue;
  }
}

/// Amortized time complexity is O(NlgN), depending on branching factor.
impl<'tree, L: Language> Iterator for Post<'tree, L> {
  type Item = Node<'tree, L>;
  fn next(&mut self) -> Option<Self::Item> {
    // start_id will always be Some until the dfs terminates
    let start = self.start_id?;
    let cursor = &mut self.cursor;
    let node = Node {
      inner: cursor.node(),
      root: self.root,
    };
    // return to start
    if node.inner.id() == start {
      self.start_id = None
    } else if cursor.goto_next_sibling() {
      // try visit sibling
      dive_down(cursor);
    } else {
      // go back to parent node
      cursor.goto_parent();
    }
    Some(node)
  }
}
impl<'tree, L: Language> FusedIterator for Post<'tree, L> {}

impl<'t, L: Language> Traversal<'t, L> for Post<'t, L> {
  #[inline]
  fn mark_last_node_matched(&mut self, matched: bool) {
    todo!()
  }
}

/// Represents a level-order traversal.
/// It is implemented with [`VecDeque`] since quadratic backtracking is too time consuming.
/// Though level-order is not used as frequently as other DFS traversals,
/// traversing a big AST with level-order should be done with caution since it might increase the memory usage.
pub struct Level<'tree, L: Language> {
  deque: VecDeque<ts::Node<'tree>>,
  cursor: ts::TreeCursor<'tree>,
  root: &'tree Root<L>,
}

impl<'tree, L: Language> Level<'tree, L> {
  pub fn new(node: &Node<'tree, L>) -> Self {
    let mut deque = VecDeque::new();
    deque.push_back(node.inner.clone());
    let cursor = node.inner.walk();
    Self {
      deque,
      cursor,
      root: node.root,
    }
  }
}

/// Time complexity is O(N). Space complexity is O(N)
impl<'tree, L: Language> Iterator for Level<'tree, L> {
  type Item = Node<'tree, L>;
  fn next(&mut self) -> Option<Self::Item> {
    let inner = self.deque.pop_front()?;
    let children = inner.children(&mut self.cursor);
    self.deque.extend(children);
    Some(Node {
      inner,
      root: self.root,
    })
  }
}
impl<'tree, L: Language> FusedIterator for Level<'tree, L> {}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};
  use std::ops::Range;

  // recursive pre order as baseline
  fn pre_order(node: Node<Tsx>) -> Vec<Range<usize>> {
    let mut ret = vec![node.range()];
    ret.extend(node.children().flat_map(pre_order));
    ret
  }

  // recursion baseline
  fn post_order(node: Node<Tsx>) -> Vec<Range<usize>> {
    let mut ret: Vec<_> = node.children().flat_map(post_order).collect();
    ret.push(node.range());
    ret
  }

  fn pre_order_equivalent(source: &str) {
    let grep = Tsx.ast_grep(source);
    let node = grep.root();
    let iterative: Vec<_> = Pre::new(&node).map(|n| n.range()).collect();
    let recursive = pre_order(node);
    assert_eq!(iterative, recursive);
  }

  fn post_order_equivalent(source: &str) {
    let grep = Tsx.ast_grep(source);
    let node = grep.root();
    let iterative: Vec<_> = Post::new(&node).map(|n| n.range()).collect();
    let recursive = post_order(node);
    assert_eq!(iterative, recursive);
  }

  const CASES: &[&str] = &[
    "console.log('hello world')",
    "let a = (a, b, c)",
    "function test() { let a = 1; let b = 2; a === b}",
    "[[[[[[]]]]], 1 , 2 ,3]",
    "class A { test() { class B {} } }",
  ];

  #[test]
  fn tes_pre_order() {
    for case in CASES {
      pre_order_equivalent(case);
    }
  }

  #[test]
  fn test_post_order() {
    for case in CASES {
      post_order_equivalent(case);
    }
  }

  #[test]
  fn test_different_order() {
    for case in CASES {
      let grep = Tsx.ast_grep(case);
      let node = grep.root();
      let pre: Vec<_> = Pre::new(&node).map(|n| n.range()).collect();
      let post: Vec<_> = Post::new(&node).map(|n| n.range()).collect();
      let level: Vec<_> = Level::new(&node).map(|n| n.range()).collect();
      assert_ne!(pre, post);
      assert_ne!(pre, level);
      assert_ne!(post, level);
    }
  }

  #[test]
  fn test_fused_traversal() {
    for case in CASES {
      let grep = Tsx.ast_grep(case);
      let node = grep.root();
      let mut pre = Pre::new(&node);
      let mut post = Post::new(&node);
      while pre.next().is_some() {}
      while post.next().is_some() {}
      assert!(pre.next().is_none());
      assert!(pre.next().is_none());
      assert!(post.next().is_none());
      assert!(post.next().is_none());
    }
  }

  #[test]
  fn test_non_root_traverse() {
    let grep = Tsx.ast_grep("let a = 123; let b = 123;");
    let node = grep.root();
    let pre: Vec<_> = Pre::new(&node).map(|n| n.range()).collect();
    let post: Vec<_> = Post::new(&node).map(|n| n.range()).collect();
    let node2 = node.child(0).unwrap();
    let pre2: Vec<_> = Pre::new(&node2).map(|n| n.range()).collect();
    let post2: Vec<_> = Post::new(&node2).map(|n| n.range()).collect();
    // traversal should stop at node
    assert_ne!(pre, pre2);
    assert_ne!(post, post2);
    // child traversal should be a part of parent traversal
    assert!(pre[1..].starts_with(&pre2));
    assert!(post.starts_with(&post2));
  }
}
