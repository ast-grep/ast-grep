use ast_grep_core::language::TSLanguage;
use ast_grep_dynamic::{DynamicLang, Registration};
use ast_grep_language::{Language, SupportLang};
use serde::{Deserialize, Serialize};

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone)]
pub struct CustomLang {
  library_path: PathBuf,
  /// the dylib symbol to load ts-language, default is `tree_sitter_{name}`
  language_symbol: Option<String>,
  meta_var_char: Option<char>,
  expando_char: Option<char>,
  // extensions: Vec<String>,
}

#[pyfunction]
pub fn register_dynamic_language(dict: Bound<PyDict>) -> PyResult<()> {
  let langs = depythonize(dict.as_any())?;
  let base = std::env::current_dir()?;
  register(base, langs);
  Ok(())
}

fn register(base: PathBuf, langs: HashMap<String, CustomLang>) {
  let registrations = langs
    .into_iter()
    .map(|(name, custom)| to_registration(name, custom, &base))
    .collect();
  // TODO, add error handling
  unsafe { DynamicLang::register(registrations).expect("TODO") }
}

fn to_registration(name: String, custom_lang: CustomLang, base: &Path) -> Registration {
  let path = base.join(custom_lang.library_path);
  let sym = custom_lang
    .language_symbol
    .unwrap_or_else(|| format!("tree_sitter_{name}"));
  Registration {
    lang_name: name,
    lib_path: path,
    symbol: sym,
    meta_var_char: custom_lang.meta_var_char,
    expando_char: custom_lang.expando_char,
    // extensions: custom_lang.extensions,
    extensions: vec![],
  }
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
