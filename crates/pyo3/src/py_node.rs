use crate::range::Range;
use crate::SgRoot;

use ast_grep_config::{SerializableRule, SerializableRuleCore};
use ast_grep_core::{NodeMatch, StrDoc};
use ast_grep_language::SupportLang;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;

#[pyclass]
pub struct SgNode {
  pub inner: NodeMatch<'static, StrDoc<SupportLang>>,
  // refcount SgRoot
  pub(crate) root: Py<SgRoot>,
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

  fn get_match(&self, meta_var: &str) -> Option<Self> {
    self
      .inner
      .get_env()
      .get_match(meta_var)
      .cloned()
      .map(|n| Self {
        inner: NodeMatch::from(n),
        root: self.root.clone(),
      })
  }

  fn get_multiple_matches(&self, meta_var: &str) -> Vec<SgNode> {
    self
      .inner
      .get_env()
      .get_multiple_matches(meta_var)
      .into_iter()
      .map(|n| Self {
        inner: NodeMatch::from(n),
        root: self.root.clone(),
      })
      .collect()
  }

  /*---------- Tree Traversal  ----------*/
  fn get_root(&self) -> Py<SgRoot> {
    self.root.clone()
  }

  #[pyo3(signature = (config=None, **kwargs))]
  fn find(&self, config: Option<&PyDict>, kwargs: Option<&PyDict>) -> Option<Self> {
    let config = self.get_config(config, kwargs);
    let matcher = config.get_matcher(&Default::default()).unwrap();
    let inner = self.inner.find(matcher)?;
    Some(Self {
      inner,
      root: self.root.clone(),
    })
  }
  #[pyo3(signature = (config=None, **kwargs))]
  fn find_all(&self, config: Option<&PyDict>, kwargs: Option<&PyDict>) -> Vec<Self> {
    let config = self.get_config(config, kwargs);
    let matcher = config.get_matcher(&Default::default()).unwrap();
    self
      .inner
      .find_all(matcher)
      .map(|n| Self {
        inner: n,
        root: self.root.clone(),
      })
      .collect()
  }

  fn field(&self, name: &str) -> Option<SgNode> {
    self.inner.field(name).map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn parent(&self) -> Option<SgNode> {
    self.inner.parent().map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn child(&self, nth: usize) -> Option<SgNode> {
    self.inner.child(nth).map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn ancestors(&self) -> Vec<SgNode> {
    self
      .inner
      .ancestors()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }

  fn next(&self) -> Option<SgNode> {
    self.inner.next().map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn next_all(&self) -> Vec<SgNode> {
    self
      .inner
      .next_all()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }

  fn prev(&self) -> Option<SgNode> {
    self.inner.prev().map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn prev_all(&self) -> Vec<SgNode> {
    self
      .inner
      .prev_all()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }
}

impl SgNode {
  fn get_config(
    &self,
    config: Option<&PyDict>,
    kwargs: Option<&PyDict>,
  ) -> SerializableRuleCore<SupportLang> {
    let lang = self.inner.lang();
    if let Some(config) = config {
      config_from_dict(lang, config)
    } else {
      let rule = rule_from_dict(kwargs.unwrap());
      SerializableRuleCore {
        language: *lang,
        rule,
        constraints: None,
        utils: None,
        transform: None,
      }
    }
  }
}

fn config_from_dict(lang: &SupportLang, dict: &PyDict) -> SerializableRuleCore<SupportLang> {
  dict.set_item("language", lang.to_string()).unwrap();
  depythonize(dict).unwrap()
}

fn rule_from_dict(dict: &PyDict) -> SerializableRule {
  depythonize(dict).unwrap()
}
