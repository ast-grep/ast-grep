#![cfg(not(test))]
#![cfg(feature = "python")]
mod py_node;
mod range;
use py_node::SgNode;
use range::{Pos, Range};

use ast_grep_core::{AstGrep, Language, NodeMatch, StrDoc};
use ast_grep_language::SupportLang;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn ast_grep_py(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
  m.add_class::<SgRoot>()?;
  m.add_class::<SgNode>()?;
  m.add_class::<Range>()?;
  m.add_class::<Pos>()?;
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
    let inner = NodeMatch::from(tree.root());
    SgNode {
      inner,
      root: slf.into(),
    }
  }

  fn filename(&self) -> &str {
    &self.filename
  }
}
