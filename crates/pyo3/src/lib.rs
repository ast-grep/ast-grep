#![cfg(not(test))]
#![cfg(feature = "python")]
mod range;
use range::Range;

use ast_grep_config::{SerializableRule, SerializableRuleCore};
use ast_grep_core::{AstGrep, Language, Node, StrDoc};
use ast_grep_language::SupportLang;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;

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
  fn range(&self) -> Range {
    Range::from(&self.inner)
  }

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
  // TODO get_root

  #[pyo3(signature = (config=None, **kwargs))]
  fn find(&self, config: Option<&PyDict>, kwargs: Option<&PyDict>) -> Option<Self> {
    let lang = self.inner.lang();
    let config = if let Some(config) = config {
      config_from_dict(lang, config)
    } else {
      let rule = rule_from_dict(kwargs?);
      SerializableRuleCore {
        language: *lang,
        rule,
        constraints: None,
        utils: None,
        transform: None,
      }
    };
    let matcher = config.get_matcher(&Default::default()).unwrap();
    let nm = self.inner.find(matcher)?;
    Some(Self {
      inner: nm.into(),
      root: self.root.clone(),
    })
  }

  // TODO find_all
  // TODO field
  // TODO parent
  // TODO child
  // TODO ancestors
  // TODO next
  // TODO next_all
  // TODO prev
  // TODO prev_all
}

fn config_from_dict(lang: &SupportLang, dict: &PyDict) -> SerializableRuleCore<SupportLang> {
  dict.set_item("language", lang.to_string()).unwrap();
  depythonize(dict).unwrap()
}

fn rule_from_dict(dict: &PyDict) -> SerializableRule {
  depythonize(dict).unwrap()
}
