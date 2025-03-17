use ast_grep_core::language::TSLanguage;
use ast_grep_core::Language;

use ignore::types::{Types, TypesBuilder};
use libloading::{Error as LibError, Library, Symbol};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter_native::{Language as NativeTS, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION};

use std::borrow::Cow;
use std::fs::canonicalize;
use std::path::{Path, PathBuf};
use std::ptr::{addr_of, addr_of_mut};
use std::str::FromStr;

mod custom_lang;

pub use custom_lang::CustomLang;

type LangIndex = u32;

/// Represents a tree-sitter language loaded as dynamic lib.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct DynamicLang {
  index: LangIndex,
  // inline expando char since it is used frequently
  expando: char,
}

impl DynamicLang {
  pub fn all_langs() -> Vec<Self> {
    Self::langs()
      .iter()
      .enumerate()
      .map(|(index, inner)| DynamicLang {
        index: index as LangIndex,
        expando: inner.expando_char,
      })
      .collect()
  }
  pub fn file_types(&self) -> Types {
    let mut builder = TypesBuilder::new();
    let inner = self.inner();
    let mapping = unsafe { &*addr_of!(LANG_INDEX) };
    for (ext, i) in mapping.iter() {
      if *i == self.index {
        builder
          .add(&inner.name, &format!("*.{ext}"))
          .expect("file pattern must compile");
      }
    }
    builder.select(&inner.name);
    builder.build().expect("file type must be valid")
  }
}

impl Serialize for DynamicLang {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let name = &self.inner().name;
    serializer.serialize_str(name)
  }
}

impl<'de> Deserialize<'de> for DynamicLang {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let name = String::deserialize(deserializer)?;
    DynamicLang::from_str(&name).map_err(serde::de::Error::custom)
  }
}

impl FromStr for DynamicLang {
  type Err = String;
  fn from_str(name: &str) -> Result<Self, Self::Err> {
    let langs = Self::langs();
    for (i, lang) in langs.iter().enumerate() {
      if lang.name == name {
        return Ok(DynamicLang {
          index: i as LangIndex,
          expando: lang.expando_char,
        });
      }
    }
    Err(format!("unknow language `{name}`."))
  }
}

struct Inner {
  lang: TSLanguage,
  name: String,
  meta_var_char: char,
  expando_char: char,
  // NOTE: need to hold a reference of lib to avoid cleanup
  _lib: Library,
}

#[derive(Debug, Error)]
pub enum DynamicLangError {
  #[error("cannot load lib")]
  OpenLib(#[source] LibError),
  #[error("cannot read symbol")]
  ReadSymbol(#[source] LibError),
  #[error("Incompatible tree-sitter parser version `{0}`")]
  IncompatibleVersion(usize),
  #[error("cannot get the absolute path of dynamic lib")]
  GetLibPath(#[from] std::io::Error),
}

/// # Safety: we must keep lib in memory after load it.
/// libloading will do cleanup if `Library` is dropped which makes any lib symbol null pointer.
/// This is not desirable for our case.
unsafe fn load_ts_language(
  path: PathBuf,
  name: String,
) -> Result<(Library, TSLanguage), DynamicLangError> {
  let abs_path = canonicalize(path)?;
  let lib = Library::new(abs_path.as_os_str()).map_err(DynamicLangError::OpenLib)?;
  // NOTE: func is a symbol with lifetime bound to `lib`.
  // If we drop lib in the scope, func will be a dangling pointer.
  let func: Symbol<unsafe extern "C" fn() -> NativeTS> = lib
    .get(name.as_bytes())
    .map_err(DynamicLangError::ReadSymbol)?;
  let lang = func();
  let version = lang.version();
  if !(MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&version) {
    Err(DynamicLangError::IncompatibleVersion(version))
  } else {
    // ATTENTION: dragon ahead
    // must hold valid reference to NativeTS
    Ok((lib, lang.into()))
  }
}

// both use vec since lang will be small
static mut DYNAMIC_LANG: Vec<Inner> = vec![];
static mut LANG_INDEX: Vec<(String, u32)> = vec![];

#[derive(Default)]
pub struct Registration {
  pub lang_name: String,
  pub lib_path: PathBuf,
  pub symbol: String,
  pub meta_var_char: Option<char>,
  pub expando_char: Option<char>,
  pub extensions: Vec<String>,
}

impl DynamicLang {
  /// # Safety
  /// the register function should be called exactly once before use.
  /// It relies on a global mut static variable to be initialized.
  pub unsafe fn register(regs: Vec<Registration>) -> Result<(), DynamicLangError> {
    debug_assert!(Self::langs().is_empty());
    let mut langs = vec![];
    let mut mapping = vec![];
    for reg in regs {
      Self::register_one(reg, &mut langs, &mut mapping)?;
    }
    _ = std::mem::replace(&mut *addr_of_mut!(DYNAMIC_LANG), langs);
    _ = std::mem::replace(&mut *addr_of_mut!(LANG_INDEX), mapping);
    Ok(())
  }

  pub fn name(&self) -> &str {
    &self.inner().name
  }

  fn register_one(
    reg: Registration,
    langs: &mut Vec<Inner>,
    mapping: &mut Vec<(String, LangIndex)>,
  ) -> Result<(), DynamicLangError> {
    // lib must be retained!!
    let (_lib, lang) = unsafe { load_ts_language(reg.lib_path, reg.symbol)? };
    let meta_var_char = reg.meta_var_char.unwrap_or('$');
    let expando_char = reg.expando_char.unwrap_or(meta_var_char);
    let inner = Inner {
      name: reg.lang_name,
      lang,
      meta_var_char,
      expando_char,
      _lib,
    };
    langs.push(inner);
    let idx = langs.len() as LangIndex - 1;
    for ext in reg.extensions {
      mapping.push((ext, idx));
    }
    Ok(())
  }
  fn inner(&self) -> &Inner {
    let langs = Self::langs();
    &langs[self.index as usize]
  }

  fn langs() -> &'static Vec<Inner> {
    unsafe { &*addr_of!(DYNAMIC_LANG) }
  }
}

impl Language for DynamicLang {
  /// tree sitter language to parse the source
  fn get_ts_language(&self) -> TSLanguage {
    self.inner().lang.clone()
  }

  fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
    let ext = path.as_ref().extension()?.to_str()?;
    let mapping = unsafe { &*addr_of!(LANG_INDEX) };
    let langs = Self::langs();
    mapping.iter().find_map(|(p, idx)| {
      if p == ext {
        let index = *idx;
        Some(Self {
          index,
          expando: langs[*idx as usize].expando_char,
        })
      } else {
        None
      }
    })
  }

  /// normalize pattern code before matching
  /// e.g. remove expression_statement, or prefer parsing {} to object over block
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    if self.meta_var_char() == self.expando_char() {
      return Cow::Borrowed(query);
    };
    // use stack buffer to reduce allocation
    let mut buf = [0; 4];
    let expando = self.expando_char().encode_utf8(&mut buf);
    // TODO: use more precise replacement
    let replaced = query.replace(self.meta_var_char(), expando);
    Cow::Owned(replaced)
  }

  /// Configure meta variable special character
  /// By default $ is the metavar char, but in PHP it can be #
  #[inline]
  fn meta_var_char(&self) -> char {
    self.inner().meta_var_char
  }

  /// Some language does not accept $ as the leading char for identifiers.
  /// We need to change $ to other char at run-time to make parser happy, thus the name expando.
  /// By default this is the same as meta_var char so replacement is done at runtime.
  #[inline]
  fn expando_char(&self) -> char {
    self.expando
  }
}

#[cfg(test)]
mod test {
  use super::*;

  // currently we only have json parser for these platforms
  // apple silicon macos and linux x86_64
  fn get_tree_sitter_path() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
      "../../benches/fixtures/json-mac.so"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
      "../../benches/fixtures/json-linux.so"
    } else {
      ""
    }
  }

  #[test]
  fn test_load_parser() {
    let path = get_tree_sitter_path();
    // skip unsupported platform
    if path.is_empty() {
      return;
    }
    let (_lib, lang) = unsafe { load_ts_language(path.into(), "tree_sitter_json".into()).unwrap() };
    let sg = lang.ast_grep("{\"a\": 123}");
    assert_eq!(
      sg.root().to_sexp(),
      "(document (object (pair key: (string (string_content)) value: (number))))"
    );
  }

  #[test]
  fn test_register_lang() {
    let path = get_tree_sitter_path();
    // skip unsupported platform
    if path.is_empty() {
      return;
    }
    let registration = Registration {
      lang_name: "json".to_string(),
      expando_char: Some('_'),
      extensions: vec!["json".into()],
      lib_path: PathBuf::from(path),
      meta_var_char: None,
      symbol: "tree_sitter_json".into(),
    };
    unsafe {
      DynamicLang::register(vec![registration]).expect("should succeed");
    }
    let langs = DynamicLang::all_langs();
    assert_eq!(langs.len(), 1);
    let lang = langs[0];
    let file_types = lang.file_types();
    assert!(file_types.matched("test.json", false).is_whitelist());
    assert_eq!(lang.name(), "json");
    let sg = lang.ast_grep("{\"test\": 123}");
    assert!(sg.root().find("123").is_some());
    let parsed = DynamicLang::from_str("json").expect("ok");
    assert_eq!(parsed.index, lang.index);
  }
}
