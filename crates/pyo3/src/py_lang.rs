use anyhow::Context;
use ast_grep_core::language::TSLanguage;
use ast_grep_dynamic::{CustomLang, DynamicLang};
use ast_grep_language::{Language, SupportLang};
use serde::{Deserialize, Serialize};

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

// we need this because of different casing in python
// in Python, every field is in snake_case
// but in napi/YAML, every field is in camelCase
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomPyLang {
  library_path: PathBuf,
  /// the dylib symbol to load ts-language, default is `tree_sitter_{name}`
  language_symbol: Option<String>,
  meta_var_char: Option<char>,
  expando_char: Option<char>,
  extensions: Vec<String>,
}

impl From<CustomPyLang> for CustomLang {
  fn from(c: CustomPyLang) -> Self {
    CustomLang {
      library_path: c.library_path,
      language_symbol: c.language_symbol,
      meta_var_char: c.meta_var_char,
      expando_char: c.expando_char,
      extensions: c.extensions,
    }
  }
}

#[pyfunction]
pub fn register_dynamic_language(dict: Bound<PyDict>) -> PyResult<()> {
  let langs: HashMap<String, CustomPyLang> = depythonize(dict.as_any())?;
  let langs = langs
    .into_iter()
    .map(|(name, custom)| (name, CustomLang::from(custom)))
    .collect();
  let base = std::env::current_dir()?;
  CustomLang::register(&base, langs).context("registering dynamic language failed")?;
  Ok(())
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum PyLang {
  // inlined support lang expando char
  Builtin(SupportLang),
  Custom(DynamicLang),
}
#[derive(Debug)]
pub enum PyLangErr {
  LanguageNotSupported(String),
}

impl Display for PyLangErr {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
    use PyLangErr::*;
    match self {
      LanguageNotSupported(lang) => write!(f, "{} is not supported!", lang),
    }
  }
}

impl std::error::Error for PyLangErr {}

impl FromStr for PyLang {
  type Err = PyLangErr;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if let Ok(b) = SupportLang::from_str(s) {
      Ok(PyLang::Builtin(b))
    } else if let Ok(c) = DynamicLang::from_str(s) {
      Ok(PyLang::Custom(c))
    } else {
      Err(PyLangErr::LanguageNotSupported(s.into()))
    }
  }
}

use PyLang::*;
impl Language for PyLang {
  fn get_ts_language(&self) -> TSLanguage {
    match self {
      Builtin(b) => b.get_ts_language(),
      Custom(c) => c.get_ts_language(),
    }
  }

  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    match self {
      Builtin(b) => b.pre_process_pattern(query),
      Custom(c) => c.pre_process_pattern(query),
    }
  }

  #[inline]
  fn meta_var_char(&self) -> char {
    match self {
      Builtin(b) => b.meta_var_char(),
      Custom(c) => c.meta_var_char(),
    }
  }

  #[inline]
  fn expando_char(&self) -> char {
    match self {
      Builtin(b) => b.expando_char(),
      Custom(c) => c.expando_char(),
    }
  }
}
