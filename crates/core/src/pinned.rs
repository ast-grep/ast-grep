use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::node::{Node, Root};

// TODO: add comments
#[doc(hidden)]
pub struct PinnedNodeData<L: Language, T> {
  pin: Root<L>,
  data: T,
}

impl<T, L: Language + 'static> PinnedNodeData<L, T> {
  pub fn new<F>(pin: Root<L>, func: F) -> Self
  where
    F: FnOnce(&'static Root<L>) -> T,
  {
    // TODO: explain why unsafe works here and what guarantee it needs
    let reference = unsafe { &*(&pin as *const Root<L>) as &'static Root<L> };
    let data = func(reference);
    Self { pin, data }
  }
}

impl<L: Language + 'static, T> PinnedNodeData<L, T>
where
  T: NodeData<L>,
{
  pub fn get_data(&mut self) -> &T::Data<'_> {
    let pin = unsafe { &*(&self.pin as *const Root<L>) as &'static Root<L> };
    self.data.visit_nodes(|n| unsafe { pin.readopt(n) });
    self.data.get_data()
  }
}

/// # Safety
/// TODO: explain unsafe trait
pub unsafe trait NodeData<L> {
  type Data<'a>
  where
    Self: 'a;
  fn get_data(&self) -> &Self::Data<'_>;
  fn visit_nodes<F>(&mut self, f: F)
  where
    F: FnMut(&mut Node<'_, L>);
}

unsafe impl<L: Language> NodeData<L> for Node<'static, L> {
  type Data<'a> = Node<'a, L>;
  fn get_data(&self) -> &Self::Data<'_> {
    self
  }
  fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, L>),
  {
    f(self)
  }
}

unsafe impl<L: Language> NodeData<L> for NodeMatch<'static, L> {
  type Data<'a> = NodeMatch<'a, L>;
  fn get_data(&self) -> &Self::Data<'_> {
    self
  }
  fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, L>),
  {
    f(unsafe { self.get_mut_node() })
  }
}

unsafe impl<L: Language> NodeData<L> for Vec<Node<'static, L>> {
  type Data<'a> = Vec<Node<'a, L>>;
  fn get_data(&self) -> &Self::Data<'_> {
    self
  }
  fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, L>),
  {
    for n in self {
      f(n)
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::Tsx;
  use crate::node::Root;
  fn return_from_func() -> PinnedNodeData<Tsx, Node<'static, Tsx>> {
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

  fn return_vec() -> PinnedNodeData<Tsx, Vec<Node<'static, Tsx>>> {
    let root = Root::new("let a = 123", Tsx);
    PinnedNodeData::new(root, |r| r.root().child(0).unwrap().children().collect())
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
