mod injection;
mod lang_globs;

use crate::utils::ErrorContext as EC;

use anyhow::{Context, Result};
use ast_grep_core::{
  language::{TSLanguage, TSRange},
  Doc, Node,
};
use ast_grep_dynamic::DynamicLang;
use ast_grep_language::{Language, SupportLang};
use ignore::types::Types;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::Path;
use std::str::FromStr;

pub use ast_grep_dynamic::CustomLang;
pub use injection::SerializableInjection;
pub use lang_globs::LanguageGlobs;

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(untagged)]
pub enum SgLang {
  // inlined support lang expando char
  Builtin(SupportLang),
  Custom(DynamicLang),
}

impl SgLang {
  pub fn file_types(&self) -> Types {
    let default_types = match self {
      Builtin(b) => b.file_types(),
      Custom(c) => c.file_types(),
    };
    lang_globs::merge_globs(self, default_types)
  }

  // register_globs must be called after register_custom_language
  pub fn register_custom_language(base: &Path, langs: HashMap<String, CustomLang>) -> Result<()> {
    CustomLang::register(base, langs).context(EC::CustomLanguage)
  }

  // TODO: add tests
  // register_globs must be called after register_custom_language
  pub fn register_globs(langs: LanguageGlobs) -> Result<()> {
    unsafe {
      lang_globs::register(langs)?;
    }
    Ok(())
  }

  pub fn register_injections(injections: Vec<SerializableInjection>) -> Result<()> {
    unsafe { injection::register_injetables(injections) }
  }

  pub fn all_langs() -> Vec<Self> {
    let builtin = SupportLang::all_langs().iter().copied().map(Self::Builtin);
    let customs = DynamicLang::all_langs().into_iter().map(Self::Custom);
    builtin.chain(customs).collect()
  }

  pub fn injectable_sg_langs(&self) -> Option<impl Iterator<Item = Self>> {
    let langs = self.injectable_languages()?;
    // TODO: handle injected languages not found
    // e.g vue can inject scss which is not supported by sg
    // we should report an error here
    let iter = langs.iter().filter_map(|s| SgLang::from_str(s).ok());
    Some(iter)
  }

  pub fn augmented_file_type(&self) -> Types {
    let self_type = self.file_types();
    let injector = Self::all_langs().into_iter().filter_map(|lang| {
      lang
        .injectable_sg_langs()?
        .any(|l| l == *self)
        .then_some(lang)
    });
    let injector_types = injector.map(|lang| lang.file_types());
    let all_types = std::iter::once(self_type).chain(injector_types);
    lang_globs::merge_types(all_types)
  }

  pub fn file_types_for_langs(langs: impl Iterator<Item = Self>) -> Types {
    let types = langs.map(|lang| lang.augmented_file_type());
    lang_globs::merge_types(types)
  }
}

impl Display for SgLang {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    match self {
      Builtin(b) => write!(f, "{}", b),
      Custom(c) => write!(f, "{}", c.name()),
    }
  }
}

impl Debug for SgLang {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    match self {
      Builtin(b) => write!(f, "{:?}", b),
      Custom(c) => write!(f, "{:?}", c.name()),
    }
  }
}

#[derive(Debug)]
pub enum SgLangErr {
  LanguageNotSupported(String),
}

impl Display for SgLangErr {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
    use SgLangErr::*;
    match self {
      LanguageNotSupported(lang) => write!(f, "{} is not supported!", lang),
    }
  }
}

impl std::error::Error for SgLangErr {}

impl FromStr for SgLang {
  type Err = SgLangErr;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if let Ok(b) = SupportLang::from_str(s) {
      Ok(SgLang::Builtin(b))
    } else if let Ok(c) = DynamicLang::from_str(s) {
      Ok(SgLang::Custom(c))
    } else {
      Err(SgLangErr::LanguageNotSupported(s.into()))
    }
  }
}

impl From<SupportLang> for SgLang {
  fn from(value: SupportLang) -> Self {
    Self::Builtin(value)
  }
}
impl From<DynamicLang> for SgLang {
  fn from(value: DynamicLang) -> Self {
    Self::Custom(value)
  }
}

use SgLang::*;
impl Language for SgLang {
  fn get_ts_language(&self) -> TSLanguage {
    match self {
      Builtin(b) => b.get_ts_language(),
      Custom(c) => c.get_ts_language(),
    }
  }

  fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
    // respect user overriding like languageGlobs and custom lang
    // TODO: test this preference
    let path = path.as_ref();
    lang_globs::from_path(path)
      .or_else(|| DynamicLang::from_path(path).map(Custom))
      .or_else(|| SupportLang::from_path(path).map(Builtin))
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

  fn injectable_languages(&self) -> Option<&'static [&'static str]> {
    injection::injectable_languages(*self)
  }

  fn extract_injections<D: Doc>(&self, root: Node<D>) -> HashMap<String, Vec<TSRange>> {
    injection::extract_injections(root)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use std::mem::size_of;

  #[test]
  fn test_sg_lang_size() {
    assert_eq!(size_of::<SgLang>(), size_of::<DynamicLang>());
  }
}
