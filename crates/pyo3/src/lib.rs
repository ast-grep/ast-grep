#![cfg(not(test))]
#![cfg(feature = "pyo3")]
use ast_grep_core::{AstGrep, Language, Node, StrDoc};
use ast_grep_language::SupportLang;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn ast_grep_pyo3(_py: Python, m: &PyModule) -> PyResult<()> {
  m.add_class::<SgRoot>()?;
  m.add_class::<SgNode>()?;
  Ok(())
}

#[pyclass]
struct SgRoot {
  inner: AstGrep<StrDoc<SupportLang>>,
  filename: String,
}

#[pymethods]
impl SgRoot {
  #[new]
  fn new(src: &str, lang: &str) -> Self {
    let lang: SupportLang = lang.parse().unwrap();
    let inner = lang.ast_grep(src);
    Self {
      inner,
      filename: "anonymous".into(),
    }
  }

  fn root(slf: PyRef<Self>) -> SgNode {
    let tree = unsafe { &*(&slf.inner as *const AstGrep<_>) } as &'static AstGrep<_>;
    let inner = tree.root();
    SgNode {
      inner,
      root: slf.into(),
    }
  }

  fn filename(&self) -> &str {
    &self.filename
  }
}

#[pyclass]
struct SgNode {
  inner: Node<'static, StrDoc<SupportLang>>,
  // refcount SgRoot
  root: Py<SgRoot>,
}

// it is safe to send tree-sitter Node
// because it is refcnt and concurrency safe
unsafe impl Send for SgNode {}

#[pymethods]
impl SgNode {
  /*----------  Node Inspection ----------*/
  // TODO range

  fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }

  fn is_named(&self) -> bool {
    self.inner.is_named()
  }

  fn is_named_leaf(&self) -> bool {
    self.inner.is_named_leaf()
  }

  fn kind(&self) -> String {
    self.inner.kind().to_string()
  }

  fn text(&self) -> String {
    self.inner.text().to_string()
  }

  /*---------- Search Refinement  ----------*/
  fn matches(&self, m: String) -> bool {
    self.inner.matches(&*m)
  }

  fn inside(&self, m: String) -> bool {
    self.inner.inside(&*m)
  }

  fn has(&self, m: String) -> bool {
    self.inner.has(&*m)
  }

  fn precedes(&self, m: String) -> bool {
    self.inner.precedes(&*m)
  }

  fn follows(&self, m: String) -> bool {
    self.inner.follows(&*m)
  }

  // TODO get_match
  // TODO get_multiple_matches

  /*---------- Tree Traversal  ----------*/
  fn find(&self, pattern: String) -> Option<Self> {
    let nm = self.inner.find(&*pattern)?;
    Some(Self {
      inner: nm.into(),
      root: self.root.clone(),
    })
  }
}
