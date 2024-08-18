use crate::matcher::NodeMatch;
use crate::node::{Node, Root};
use crate::Doc;

// ast-grep Node contains a reference to Root. It implies that
// node can be used only when the Root is valid and not dropped.
// By default, tree-sitter Node<'r> is scoped by ast Root's lifetime
// That is, Node can be only used when root is on the call stack (RAII)
// It is usually sufficient but for following scenario the brwchck is too conservative:
// 1. passing Root and Node across threads
// 2. passing Root and Node across FFI boundary (from Rust to napi/pyo3)
//
// This resembles self-referencing pattern and we can use solution similar to PinBox.
// Actually, tree-sitter's Node reference is already pointing to a heap address.
// N.B. it is not documented but can be inferred from the source code and concurrency doc paragraph.
// https://github.com/tree-sitter/tree-sitter/blob/20924fa4cdeb10d82ac308481e39bf8519334e55/lib/src/tree.c#L9-L20
// https://github.com/tree-sitter/tree-sitter/blob/20924fa4cdeb10d82ac308481e39bf8519334e55/lib/src/tree.c#L37-L39
// https://tree-sitter.github.io/tree-sitter/using-parsers#concurrency
//
// So **as long as Root is not dropped, the Tree will not be freed. And Node will be valid.**
//
// PinnedNodeData provides a systematical way to keep Root live and `T` can be anything containing valid Nodes.
// Nodes' lifetime is 'static, meaning the Node is not borrow checked instead of living throughout the program.
// There are two ways to use PinnedNodeData
// 1. use it by borrowing. PinnedNodeData guarantees Root is alive and Node in T is valid.
//    Notable example is sending Node across threads.
// 2. take its ownership. Users should take extra care to keep Root alive.
//    Notable example is sending Root to JavaScript/Python heap.
#[doc(hidden)]
pub struct PinnedNodeData<D: Doc, T> {
  pin: Root<D>,
  data: T,
}

impl<T, D: Doc + 'static> PinnedNodeData<D, T> {
  pub fn new<F>(pin: Root<D>, func: F) -> Self
  where
    F: FnOnce(&'static Root<D>) -> T,
  {
    // TODO: explain why unsafe works here and what guarantee it needs
    let reference = unsafe { &*(&pin as *const Root<D>) as &'static Root<D> };
    let data = func(reference);
    Self { pin, data }
  }
}

impl<D: Doc + 'static, T> PinnedNodeData<D, T>
where
  T: NodeData<D>,
{
  pub fn get_data(&mut self) -> &T::Data<'_> {
    let pin = unsafe { &*(&self.pin as *const Root<D>) as &'static Root<D> };
    self.data.visit_nodes(|n| unsafe { pin.readopt(n) });
    self.data.get_data()
  }
  pub fn into_raw(self) -> (Root<D>, T) {
    (self.pin, self.data)
  }
}

/// # Safety
/// TODO: explain unsafe trait
pub unsafe trait NodeData<D> {
  type Data<'a>
  where
    Self: 'a;
  fn get_data(&self) -> &Self::Data<'_>;
  fn visit_nodes<F>(&mut self, f: F)
  where
    F: FnMut(&mut Node<'_, D>);
}

unsafe impl<D: Doc> NodeData<D> for Node<'static, D> {
  type Data<'a> = Node<'a, D>;
  fn get_data(&self) -> &Self::Data<'_> {
    self
  }
  fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, D>),
  {
    f(self)
  }
}

unsafe impl<D: Doc> NodeData<D> for NodeMatch<'static, D> {
  type Data<'a> = NodeMatch<'a, D>;
  fn get_data(&self) -> &Self::Data<'_> {
    self
  }
  fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, D>),
  {
    // update the matched Node
    f(unsafe { self.get_node_mut() });
    // update the meta variable captured
    let env = self.get_env_mut();
    env.visit_nodes(f);
  }
}

unsafe impl<D: Doc> NodeData<D> for Vec<NodeMatch<'static, D>> {
  type Data<'a> = Vec<NodeMatch<'a, D>>;
  fn get_data(&self) -> &Self::Data<'_> {
    self
  }
  fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, D>),
  {
    for n in self {
      n.visit_nodes(&mut f)
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::node::Root;
  use crate::StrDoc;

  fn return_from_func() -> PinnedNodeData<StrDoc<Tsx>, Node<'static, StrDoc<Tsx>>> {
    let root = Root::new("let a = 123", Tsx);
    PinnedNodeData::new(root, |r| r.root().child(0).unwrap().child(1).unwrap())
  }

  #[test]
  fn test_borrow() {
    let mut retained = return_from_func();
    let b = retained.get_data();
    assert_eq!(b.text(), "a = 123");
    assert!(matches!(b.lang(), Tsx));
  }

  #[test]
  #[ignore]
  fn test_node_match() {
    todo!()
  }

  fn return_vec() -> PinnedNodeData<StrDoc<Tsx>, Vec<NodeMatch<'static, StrDoc<Tsx>>>> {
    let root = Root::new("let a = 123", Tsx);
    PinnedNodeData::new(root, |r| {
      r.root()
        .child(0)
        .unwrap()
        .children()
        .map(NodeMatch::from)
        .collect()
    })
  }

  #[test]
  fn test_vec_node() {
    let mut pinned = return_vec();
    let nodes = pinned.get_data();
    assert!(!nodes.is_empty());
    assert_eq!(nodes[0].text(), "let");
    assert_eq!(nodes[1].text(), "a = 123");
  }
}
