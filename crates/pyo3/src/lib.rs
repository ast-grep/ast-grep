#![cfg(not(test))]
#![cfg(feature = "python")]
mod py_lang;
mod py_node;
mod range;
mod unicode_position;
use py_lang::register_dynamic_language;
use py_node::{Edit, SgNode};
use range::{Pos, Range};

use ast_grep_core::{AstGrep, Language, NodeMatch, StrDoc};
use py_lang::PyLang;
use pyo3::prelude::*;

use unicode_position::UnicodePosition;

/// A Python module implemented in Rust.
#[pymodule]
fn ast_grep_py(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
  m.add_class::<SgRoot>()?;
  m.add_class::<SgNode>()?;
  m.add_class::<Range>()?;
  m.add_class::<Pos>()?;
  m.add_class::<Edit>()?;
  m.add_function(wrap_pyfunction!(register_dynamic_language, m)?)?;
  Ok(())
}

#[pyclass]
struct SgRoot {
  inner: AstGrep<StrDoc<PyLang>>,
  filename: String,
  pub(crate) position: UnicodePosition,
}

#[pymethods]
impl SgRoot {
  #[new]
  fn new(src: &str, lang: &str) -> Self {
    let position = UnicodePosition::new(src);
    let lang: PyLang = lang.parse().unwrap();
    let inner = lang.ast_grep(src);
    Self {
      inner,
      filename: "anonymous".into(),
      position,
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
