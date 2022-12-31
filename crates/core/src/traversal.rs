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
//! It is also possible to specify whether the Traversal should visit nested matches.
//! For example, suppose we want to find all usages of calling `foo` in the source `foo(foo())`.
//! The code has two matching calls and we can configure a traversal
//! to report only the inner one, only the outer one or both.
//!
//! Pre and Post order traversals in this module are implemented using tree-sitter's cursor API without extra heap allocation.
//! It is recommended to use traversal instead of tree recursion to avoid stack overflow and memory overhead.
//! Level order is also included for completeness and should be used sparingly.

use crate::language::Language;
use crate::{Node, Root};
use std::collections::VecDeque;
use std::iter::FusedIterator;
use tree_sitter as ts;

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
