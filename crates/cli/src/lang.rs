mod alias_lang;
mod custom_lang;

use ast_grep_core::language::TSLanguage;
use ast_grep_dynamic::DynamicLang;
use ast_grep_language::{Language, SupportLang};
use ignore::types::Types;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use alias_lang::AliasLang;
pub use alias_lang::AliasRegistration;
pub use custom_lang::CustomLang;

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SgLang {
  Alias(AliasLang),
  // inlined support lang expando char
  Builtin(SupportLang),
  Custom(DynamicLang),
}

impl SgLang {
  pub fn file_types(&self) -> Types {
    match self {
      Alias(a) => a.file_types(),
      Builtin(b) => b.file_types(),
      Custom(c) => c.file_types(),
    }
  }

  pub fn register_custom_language(base: PathBuf, langs: HashMap<String, CustomLang>) {
    CustomLang::register(base, langs)
  }

  pub fn register_alias_language(langs: HashMap<String, AliasRegistration>) {
    unsafe { AliasLang::register(langs) }
  }

  pub fn all_langs() -> Vec<Self> {
    let aliases = AliasLang::all_langs().into_iter().map(Self::Alias);
    let builtin = SupportLang::all_langs().iter().copied().map(Self::Builtin);
    let customs = DynamicLang::all_langs().into_iter().map(Self::Custom);
    builtin.chain(customs).chain(aliases).collect()
  }
}

impl Debug for SgLang {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Alias(a) => write!(f, "{}", a.name()),
      Builtin(b) => write!(f, "{}", b),
      Custom(c) => write!(f, "{}", c.name()),
    }
  }
}

impl Display for SgLang {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    match self {
      Alias(a) => write!(f, "{:?}", a.name()),
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

use SgLang::*;
impl Language for SgLang {
  fn get_ts_language(&self) -> TSLanguage {
    match self {
      Alias(a) => a.get_ts_language(),
      Builtin(b) => b.get_ts_language(),
      Custom(c) => c.get_ts_language(),
    }
  }

  fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
    SupportLang::from_path(path.as_ref())
      .map(SgLang::from)
      .or_else(|| DynamicLang::from_path(path.as_ref()).map(SgLang::Custom))
      .or_else(|| AliasLang::from_path(path).map(SgLang::Alias))
  }

  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    match self {
      Alias(a) => a.pre_process_pattern(query),
      Builtin(b) => b.pre_process_pattern(query),
      Custom(c) => c.pre_process_pattern(query),
    }
  }

  #[inline]
  fn meta_var_char(&self) -> char {
    match self {
      Alias(a) => a.meta_var_char(),
      Builtin(b) => b.meta_var_char(),
      Custom(c) => c.meta_var_char(),
    }
  }

  #[inline]
  fn expando_char(&self) -> char {
    match self {
      Alias(a) => a.expando_char(),
      Builtin(b) => b.expando_char(),
      Custom(c) => c.expando_char(),
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
