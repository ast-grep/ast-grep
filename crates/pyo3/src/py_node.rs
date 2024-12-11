use crate::py_lang::PyLang;
use crate::range::Range;
use crate::SgRoot;

use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore};
use ast_grep_core::{NodeMatch, StrDoc};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Context;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;

#[pyclass(mapping)]
pub struct SgNode {
  pub inner: NodeMatch<'static, StrDoc<PyLang>>,
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
    Python::with_gil(|py| {
      let root = self.root.bind(py);
      let root = root.borrow();
      Range::from(&self.inner, &root.position)
    })
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

  fn field_children(&self, name: &str) -> Vec<SgNode> {
    self
      .inner
      .field_children(name)
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
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

  /*---------- Edit  ----------*/
  fn replace(&self, text: &str) -> Edit {
    let byte_range = self.inner.range();
    Python::with_gil(|py| {
      let root = self.root.bind(py);
      let root = root.borrow();
      let start_pos = root.position.byte_to_char(byte_range.start);
      let end_pos = root.position.byte_to_char(byte_range.end);
      Edit {
        start_pos,
        end_pos,
        inserted_text: text.to_string(),
      }
    })
  }

  fn commit_edits(&self, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|edit| edit.start_pos);
    let mut new_content = String::new();
    let old_content = self.text();
    let converted: Vec<_> = Python::with_gil(move |py| {
      let root = self.root.bind(py);
      let root = root.borrow();
      let conv = &root.position;
      edits
        .into_iter()
        .map(|mut e| {
          e.start_pos = conv.char_to_byte(e.start_pos);
          e.end_pos = conv.char_to_byte(e.end_pos);
          e
        })
        .collect()
    });
    let offset = self.inner.range().start;
    let mut start = 0;
    for diff in converted {
      let pos = diff.start_pos - offset;
      // skip overlapping edits
      if start > pos {
        continue;
      }
      new_content.push_str(&old_content[start..pos]);
      new_content.push_str(&diff.inserted_text);
      start = diff.end_pos - offset;
    }
    // add trailing statements
    new_content.push_str(&old_content[start..]);
    new_content
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
  ) -> PyResult<RuleCore<PyLang>> {
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
  Ok(depythonize(dict.as_any())?)
}

fn config_from_rule(dict: Bound<PyDict>) -> PyResult<SerializableRuleCore> {
  let rule = depythonize(dict.as_any())?;
  Ok(SerializableRuleCore {
    rule,
    constraints: None,
    utils: None,
    transform: None,
    fix: None,
  })
}

fn get_matcher_from_rule(lang: &PyLang, dict: Option<Bound<PyDict>>) -> PyResult<RuleCore<PyLang>> {
  let rule = dict.ok_or_else(|| PyErr::new::<PyValueError, _>("rule must not be empty"))?;
  let env = DeserializeEnv::new(*lang);
  let config = config_from_rule(rule)?;
  let matcher = config.get_matcher(env).context("cannot get matcher")?;
  Ok(matcher)
}

#[pyclass(get_all, set_all)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Edit {
  /// The start position of the edit in character
  pub start_pos: usize,
  /// The end position of the edit in character
  pub end_pos: usize,
  /// The text to be inserted
  pub inserted_text: String,
}
