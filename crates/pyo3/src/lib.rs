#![cfg(not(test))]
#![cfg(feature = "pyo3")]
use ast_grep_core::{pinned::PinnedNodeData, AstGrep, Language, Node, StrDoc};
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
  fn to_sexp(&self, py: Python) -> String {
    self.inner.to_sexp().to_string()
  }
}
