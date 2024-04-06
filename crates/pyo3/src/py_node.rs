use crate::range::Range;
use crate::SgRoot;

use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore};
use ast_grep_core::{NodeMatch, StrDoc};
use ast_grep_language::SupportLang;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Context;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize_bound;

#[pyclass(mapping)]
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
  #[pyo3(signature = (**kwargs))]
  fn matches(&self, kwargs: Option<Bound<PyDict>>) -> PyResult<bool> {
    let matcher = get_matcher_from_rule(self.inner.lang(), kwargs)?;
    Ok(self.inner.matches(matcher))
  }

  #[pyo3(signature = (**kwargs))]
  fn inside(&self, kwargs: Option<Bound<PyDict>>) -> PyResult<bool> {
    let matcher = get_matcher_from_rule(self.inner.lang(), kwargs)?;
    Ok(self.inner.inside(matcher))
  }

  #[pyo3(signature = (**kwargs))]
  fn has(&self, kwargs: Option<Bound<PyDict>>) -> PyResult<bool> {
    let matcher = get_matcher_from_rule(self.inner.lang(), kwargs)?;
    Ok(self.inner.has(matcher))
  }

  #[pyo3(signature = (**kwargs))]
  fn precedes(&self, kwargs: Option<Bound<PyDict>>) -> PyResult<bool> {
    let matcher = get_matcher_from_rule(self.inner.lang(), kwargs)?;
    Ok(self.inner.precedes(matcher))
  }

  #[pyo3(signature = (**kwargs))]
  fn follows(&self, kwargs: Option<Bound<PyDict>>) -> PyResult<bool> {
    let matcher = get_matcher_from_rule(self.inner.lang(), kwargs)?;
    Ok(self.inner.follows(matcher))
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

  fn get_transformed(&self, meta_var: &str) -> Option<String> {
    self
      .inner
      .get_env()
      .get_transformed(meta_var)
      .map(|n| String::from_utf8_lossy(n).to_string())
  }

  /*---------- Tree Traversal  ----------*/
  fn get_root(&self) -> Py<SgRoot> {
    self.root.clone()
  }

  #[pyo3(signature = (config=None, **rule))]
  fn find(
    &self,
    config: Option<Bound<PyDict>>,
    rule: Option<Bound<PyDict>>,
  ) -> PyResult<Option<Self>> {
    let matcher = self.get_matcher(config, rule)?;
    if let Some(inner) = self.inner.find(matcher) {
      Ok(Some(Self {
        inner,
        root: self.root.clone(),
      }))
    } else {
      Ok(None)
    }
  }

  #[pyo3(signature = (config=None, **rule))]
  fn find_all(
    &self,
    config: Option<Bound<PyDict>>,
    rule: Option<Bound<PyDict>>,
  ) -> PyResult<Vec<Self>> {
    let matcher = self.get_matcher(config, rule)?;
    Ok(
      self
        .inner
        .find_all(matcher)
        .map(|n| Self {
          inner: n,
          root: self.root.clone(),
        })
        .collect(),
    )
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

  fn children(&self) -> Vec<SgNode> {
    self
      .inner
      .children()
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

  /*---------- Magic Method  ----------*/
  fn __hash__(&self) -> u64 {
    let mut s = DefaultHasher::new();
    self.inner.node_id().hash(&mut s);
    s.finish()
  }
  fn __eq__(&self, other: &Self) -> bool {
    self.inner.node_id() == other.inner.node_id()
  }
  fn __str__(&self) -> String {
    let range = self.range();
    format!("{}@{}", self.inner.kind(), range)
  }
  fn __repr__(&self) -> String {
    let range = self.range();
    let chars: Vec<_> = self.text().chars().take(10).collect();
    let src = if chars.len() > 9 {
      let s: String = chars.into_iter().take(5).collect();
      format!("{}...", s)
    } else {
      chars.into_iter().collect()
    };
    format!("SgNode(`{src}`, kind={}, range={range})", self.inner.kind())
  }
  fn __getitem__(&self, key: &str) -> PyResult<Self> {
    if let Some(node) = self.get_match(key) {
      Ok(node)
    } else {
      Err(PyErr::new::<PyKeyError, _>(key.to_string()))
    }
  }
}

impl SgNode {
  fn get_matcher(
    &self,
    config: Option<Bound<PyDict>>,
    kwargs: Option<Bound<PyDict>>,
  ) -> PyResult<RuleCore<SupportLang>> {
    let lang = self.inner.lang();
    let config = if let Some(config) = config {
      config_from_dict(config)?
    } else if let Some(rule) = kwargs {
      config_from_rule(rule)?
    } else {
      return Err(PyErr::new::<PyValueError, _>("rule must not be empty"));
    };
    let env = DeserializeEnv::new(*lang);
    let matcher = config.get_matcher(env).context("cannot get matcher")?;
    Ok(matcher)
  }
}

fn config_from_dict(dict: Bound<PyDict>) -> PyResult<SerializableRuleCore> {
  Ok(depythonize_bound(dict.into_any())?)
}

fn config_from_rule(dict: Bound<PyDict>) -> PyResult<SerializableRuleCore> {
  let rule = depythonize_bound(dict.into_any())?;
  Ok(SerializableRuleCore {
    rule,
    constraints: None,
    utils: None,
    transform: None,
    fix: None,
  })
}

fn get_matcher_from_rule(
  lang: &SupportLang,
  dict: Option<Bound<PyDict>>,
) -> PyResult<RuleCore<SupportLang>> {
  let rule = dict.ok_or_else(|| PyErr::new::<PyValueError, _>("rule must not be empty"))?;
  let env = DeserializeEnv::new(*lang);
  let config = config_from_rule(rule)?;
  let matcher = config.get_matcher(env).context("cannot get matcher")?;
  Ok(matcher)
}
