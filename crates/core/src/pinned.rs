use crate::matcher::NodeMatch;
use crate::node::{Node, Root};
use crate::Doc;

// TODO: add comments
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
    f(unsafe { self.get_node_mut() })
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
      f(unsafe { n.get_node_mut() })
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
