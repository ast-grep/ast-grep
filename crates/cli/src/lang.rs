mod custom_lang;
mod lang_globs;

use anyhow::Result;
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
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub use custom_lang::CustomLang;
pub use lang_globs::LanguageGlobs;

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    lang_globs::merge_types(self, default_types)
  }

  // register_globs must be called after register_custom_language
  pub fn register_custom_language(base: PathBuf, langs: HashMap<String, CustomLang>) {
    CustomLang::register(base, langs)
  }

  // TODO: add tests
  // register_globs must be called after register_custom_language
  pub fn register_globs(langs: LanguageGlobs) -> Result<()> {
    unsafe {
      lang_globs::register(langs)?;
    }
    Ok(())
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
    let iter = langs.iter().filter_map(|s| SgLang::from_str(s).ok());
    Some(iter)
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
    match self {
      Builtin(b) => b.injectable_languages(),
      Custom(c) => c.injectable_languages(),
    }
  }

  fn extract_injections<D: Doc>(
    &self,
    root: Node<D>,
    conv: impl Fn(Self) -> D::Lang,
  ) -> HashMap<String, Vec<TSRange>> {
    match self {
      Builtin(b) => b.extract_injections(root, |b| conv(Builtin(b))),
      Custom(c) => c.extract_injections(root, |c| conv(Custom(c))),
    }
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
