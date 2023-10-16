#![cfg(not(test))]
#![cfg(feature = "pyo3")]
use ast_grep_core::{AstGrep, Language, StrDoc};
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

  fn root(&self) -> SgNode {
    SgNode {}
  }

  fn filename(&self) -> &str {
    &self.filename
  }
}

#[pyclass]
struct SgNode {}
