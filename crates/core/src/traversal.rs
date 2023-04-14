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

use crate::matcher::Matcher;
use crate::{Doc, Node, NodeMatch, Root};

use tree_sitter as ts;

use std::collections::VecDeque;
use std::iter::FusedIterator;
use std::marker::PhantomData;

pub struct Visitor<M, A = PreOrder> {
  /// Whether a node will match if it contains or is contained in another match.
  reentrant: bool,
  /// Whether visit named node only
  named_only: bool,
  /// optional matcher to filter nodes
  matcher: M,
  /// The algorithm to traverse the tree, can be pre/post/level order
  algorithm: PhantomData<A>,
}

impl<M> Visitor<M> {
  pub fn new(matcher: M) -> Visitor<M> {
    Visitor {
      reentrant: true,
      named_only: false,
      matcher,
      algorithm: PhantomData,
    }
  }
}

impl<M, A> Visitor<M, A> {
  pub fn algorithm<Algo>(self) -> Visitor<M, Algo> {
    Visitor {
      reentrant: self.reentrant,
      named_only: self.named_only,
      matcher: self.matcher,
      algorithm: PhantomData,
    }
  }

  pub fn reentrant(self, reentrant: bool) -> Self {
    Self { reentrant, ..self }
  }

  pub fn named_only(self, named_only: bool) -> Self {
    Self { named_only, ..self }
  }
}

impl<M, A> Visitor<M, A>
where
  A: Algorithm,
{
  pub fn visit<D: Doc>(self, node: Node<D>) -> Visit<'_, D, A::Traversal<'_, D>, M>
  where
    M: Matcher<D::Lang>,
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

pub struct Visit<'t, D, T, M> {
  reentrant: bool,
  named: bool,
  matcher: M,
  traversal: T,
  lang: PhantomData<&'t D>,
}
impl<'t, D, T, M> Visit<'t, D, T, M>
where
  D: Doc + 't,
  T: Traversal<'t, D>,
  M: Matcher<D::Lang>,
{
  #[inline]
  fn mark_match(&mut self, depth: Option<usize>) {
    if !self.reentrant {
      self.traversal.calibrate_for_match(depth);
    }
  }
}

impl<'t, D, T, M> Iterator for Visit<'t, D, T, M>
where
  D: Doc + 't,
  T: Traversal<'t, D>,
  M: Matcher<D::Lang>,
{
  type Item = NodeMatch<'t, D>;
  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let match_depth = self.traversal.get_current_depth();
      let node = self.traversal.next()?;
      let pass_named = !self.named || node.is_named();
      if let Some(node_match) = pass_named.then(|| self.matcher.match_node(node)).flatten() {
        self.mark_match(Some(match_depth));
        return Some(node_match);
      } else {
        self.mark_match(None);
      }
    }
  }
}

pub trait Algorithm {
  type Traversal<'t, D: 't + Doc>: Traversal<'t, D>;
  fn traverse<D: Doc>(node: Node<D>) -> Self::Traversal<'_, D>;
}

pub struct PreOrder;
impl Algorithm for PreOrder {
  type Traversal<'t, D: 't + Doc> = Pre<'t, D>;
  fn traverse<D: Doc>(node: Node<D>) -> Self::Traversal<'_, D> {
    Pre::new(&node)
  }
}
pub struct PostOrder;
impl Algorithm for PostOrder {
  type Traversal<'t, D: 't + Doc> = Post<'t, D>;
  fn traverse<D: Doc>(node: Node<D>) -> Self::Traversal<'_, D> {
    Post::new(&node)
  }
}

/// Traversal can iterate over node by using traversal algorithm.
/// The `next` method should only handle normal, reentrant iteration.
/// If reentrancy is not desired, traversal should mutate cursor in `calibrate_for_match`.
/// Visit will maintain the matched node depth so traversal does not need to use extra field.
pub trait Traversal<'t, D: Doc + 't>: Iterator<Item = Node<'t, D>> {
  /// Calibrate cursor position to skip overlapping matches.
  /// node depth will be passed if matched, otherwise None.
  fn calibrate_for_match(&mut self, depth: Option<usize>);
  /// Returns the current depth of cursor depth.
  /// Cursor depth is incremented by 1 when moving from parent to child.
  /// Cursor depth at Root node is 0.
  fn get_current_depth(&self) -> usize;
}

/// Represents a pre-order traversal
pub struct Pre<'tree, D: Doc> {
  cursor: ts::TreeCursor<'tree>,
  root: &'tree Root<D>,
  // record the starting node, if we return back to starting point
  // we should terminate the dfs.
  start_id: Option<usize>,
  current_depth: usize,
}

impl<'tree, D: Doc> Pre<'tree, D> {
  pub fn new(node: &Node<'tree, D>) -> Self {
    Self {
      cursor: node.inner.walk(),
      root: node.root,
      start_id: Some(node.inner.id()),
      current_depth: 0,
    }
  }
  fn step_down(&mut self) -> bool {
    if self.cursor.goto_first_child() {
      self.current_depth += 1;
      true
    } else {
      false
    }
  }

  // retrace back to ancestors and find next node to explore
  fn trace_up(&mut self, start: usize) {
    let cursor = &mut self.cursor;
    while cursor.node().id() != start {
      // try visit sibling nodes
      if cursor.goto_next_sibling() {
        return;
      }
      self.current_depth -= 1;
      // go back to parent node
      cursor.goto_parent();
    }
    // terminate traversal here
    self.start_id = None;
  }
}

/// Amortized time complexity is O(NlgN), depending on branching factor.
impl<'tree, D: Doc> Iterator for Pre<'tree, D> {
  type Item = Node<'tree, D>;
  // 1. Yield the node itself
  // 2. Try visit the child node until no child available
  // 3. Try visit next sibling after going back to parent
  // 4. Repeat step 3 until returning to the starting node
  fn next(&mut self) -> Option<Self::Item> {
    // start_id will always be Some until the dfs terminates
    let start = self.start_id?;
    let cursor = &mut self.cursor;
    let inner = cursor.node(); // get current node
    let ret = Some(self.root.adopt(inner));
    // try going to children first
    if self.step_down() {
      return ret;
    }
    // if no child available, go to ancestor nodes
    // until we get to the starting point
    self.trace_up(start);
    ret
  }
}
impl<'tree, D: Doc> FusedIterator for Pre<'tree, D> {}

impl<'t, D: Doc> Traversal<'t, D> for Pre<'t, D> {
  fn calibrate_for_match(&mut self, depth: Option<usize>) {
    // not entering the node, ignore
    let Some(depth) = depth else {
      return;
    };
    // if already entering sibling or traced up, ignore
    if self.current_depth <= depth {
      return;
    }
    debug_assert!(self.current_depth > depth);
    if let Some(start) = self.start_id {
      // revert the step down
      self.cursor.goto_parent();
      self.trace_up(start);
    }
  }

  #[inline]
  fn get_current_depth(&self) -> usize {
    self.current_depth
  }
}

/// Represents a post-order traversal
pub struct Post<'tree, D: Doc> {
  cursor: ts::TreeCursor<'tree>,
  root: &'tree Root<D>,
  start_id: Option<usize>,
  current_depth: usize,
  match_depth: usize,
}

/// Amortized time complexity is O(NlgN), depending on branching factor.
impl<'tree, D: Doc> Post<'tree, D> {
  pub fn new(node: &Node<'tree, D>) -> Self {
    let mut ret = Self {
      cursor: node.inner.walk(),
      root: node.root,
      start_id: Some(node.inner.id()),
      current_depth: 0,
      match_depth: 0,
    };
    ret.trace_down();
    ret
  }
  fn trace_down(&mut self) {
    while self.cursor.goto_first_child() {
      self.current_depth += 1;
    }
  }
  fn step_up(&mut self) {
    self.current_depth -= 1;
    self.cursor.goto_parent();
  }
}

/// Amortized time complexity is O(NlgN), depending on branching factor.
impl<'tree, D: Doc> Iterator for Post<'tree, D> {
  type Item = Node<'tree, D>;
  fn next(&mut self) -> Option<Self::Item> {
    // start_id will always be Some until the dfs terminates
    let start = self.start_id?;
    let cursor = &mut self.cursor;
    let node = self.root.adopt(cursor.node());
    // return to start
    if node.inner.id() == start {
      self.start_id = None
    } else if cursor.goto_next_sibling() {
      // try visit sibling
      self.trace_down();
    } else {
      self.step_up();
    }
    Some(node)
  }
}

impl<'tree, D: Doc> FusedIterator for Post<'tree, D> {}

impl<'t, D: Doc> Traversal<'t, D> for Post<'t, D> {
  fn calibrate_for_match(&mut self, depth: Option<usize>) {
    if let Some(depth) = depth {
      // Later matches' depth should always be greater than former matches.
      // because we bump match_depth in `step_up` during traversal.
      debug_assert!(depth >= self.match_depth);
      self.match_depth = depth;
      return;
    }
    // found new nodes to explore in trace_down, skip calibration.
    if self.current_depth >= self.match_depth {
      return;
    }
    let Some(start) = self.start_id else {
      return;
    };
    while self.cursor.node().id() != start {
      self.match_depth = self.current_depth;
      if self.cursor.goto_next_sibling() {
        // try visit sibling
        self.trace_down();
        return;
      }
      self.step_up();
    }
    // terminate because all ancestors are skipped
    self.start_id = None;
  }

  #[inline]
  fn get_current_depth(&self) -> usize {
    self.current_depth
  }
}

/// Represents a level-order traversal.
/// It is implemented with [`VecDeque`] since quadratic backtracking is too time consuming.
/// Though level-order is not used as frequently as other DFS traversals,
/// traversing a big AST with level-order should be done with caution since it might increase the memory usage.
pub struct Level<'tree, D: Doc> {
  deque: VecDeque<ts::Node<'tree>>,
  cursor: ts::TreeCursor<'tree>,
  root: &'tree Root<D>,
}

impl<'tree, D: Doc> Level<'tree, D> {
  pub fn new(node: &Node<'tree, D>) -> Self {
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
impl<'tree, D: Doc> Iterator for Level<'tree, D> {
  type Item = Node<'tree, D>;
  fn next(&mut self) -> Option<Self::Item> {
    let inner = self.deque.pop_front()?;
    let children = inner.children(&mut self.cursor);
    self.deque.extend(children);
    Some(self.root.adopt(inner))
  }
}
impl<'tree, D: Doc> FusedIterator for Level<'tree, D> {}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};
  use crate::StrDoc;
  use std::ops::Range;

  // recursive pre order as baseline
  fn pre_order(node: Node<StrDoc<Tsx>>) -> Vec<Range<usize>> {
    let mut ret = vec![node.range()];
    ret.extend(node.children().flat_map(pre_order));
    ret
  }

  // recursion baseline
  fn post_order(node: Node<StrDoc<Tsx>>) -> Vec<Range<usize>> {
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

  fn pre_order_with_matcher(node: Node<StrDoc<Tsx>>, matcher: &str) -> Vec<Range<usize>> {
    if node.matches(matcher) {
      vec![node.range()]
    } else {
      node
        .children()
        .flat_map(|n| pre_order_with_matcher(n, matcher))
        .collect()
    }
  }

  fn post_order_with_matcher(node: Node<StrDoc<Tsx>>, matcher: &str) -> Vec<Range<usize>> {
    let mut ret: Vec<_> = node
      .children()
      .flat_map(|n| post_order_with_matcher(n, matcher))
      .collect();
    if ret.is_empty() && node.matches(matcher) {
      ret.push(node.range());
    }
    ret
  }

  const MATCHER_CASES: &[&str] = &[
    "Some(123)",
    "Some(1, 2, Some(2))",
    "NoMatch",
    "NoMatch(Some(123))",
    "Some(1, Some(2), Some(3))",
    "Some(1, Some(2), Some(Some(3)))",
  ];

  #[test]
  fn test_pre_order_visitor() {
    let matcher = "Some($$$)";
    for case in MATCHER_CASES {
      let grep = Tsx.ast_grep(case);
      let node = grep.root();
      let recur = pre_order_with_matcher(grep.root(), matcher);
      let visit: Vec<_> = Visitor::new(matcher)
        .reentrant(false)
        .visit(node)
        .map(|n| n.range())
        .collect();
      assert_eq!(recur, visit);
    }
  }
  #[test]
  fn test_post_order_visitor() {
    let matcher = "Some($$$)";
    for case in MATCHER_CASES {
      let grep = Tsx.ast_grep(case);
      let node = grep.root();
      let recur = post_order_with_matcher(grep.root(), matcher);
      let visit: Vec<_> = Visitor::new(matcher)
        .algorithm::<PostOrder>()
        .reentrant(false)
        .visit(node)
        .map(|n| n.range())
        .collect();
      assert_eq!(recur, visit);
    }
  }

  // match a leaf node will trace_up the cursor
  #[test]
  fn test_traversal_leaf() {
    let matcher = "true";
    let case = "((((true))));true";
    let grep = Tsx.ast_grep(case);
    let recur = pre_order_with_matcher(grep.root(), matcher);
    let visit: Vec<_> = Visitor::new(matcher)
      .reentrant(false)
      .visit(grep.root())
      .map(|n| n.range())
      .collect();
    assert_eq!(recur, visit);
    let recur = post_order_with_matcher(grep.root(), matcher);
    let visit: Vec<_> = Visitor::new(matcher)
      .algorithm::<PostOrder>()
      .reentrant(false)
      .visit(grep.root())
      .map(|n| n.range())
      .collect();
    assert_eq!(recur, visit);
  }
}
