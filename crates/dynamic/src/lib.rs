use ast_grep_core::language::TSLanguage;
use ast_grep_core::Language;

use libloading::{Error as LibError, Library, Symbol};
use thiserror::Error;
use tree_sitter_native::{Language as NativeTS, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION};

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::canonicalize;
use std::path::PathBuf;

#[derive(Clone)]
pub struct DynamicLang {
  lang: TSLanguage,
  meta_var_char: char,
  expando_char: char,
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
fn load_ts_language(path: PathBuf, name: String) -> Result<TSLanguage, DynamicLangError> {
  let lang = unsafe {
    let abs_path = canonicalize(path)?;
    let lib = Library::new(abs_path.as_os_str()).map_err(DynamicLangError::OpenLib)?;
    // NOTE: func is a symbol with lifetime bound to `lib`.
    // If we drop lib in the scope, func will be a dangling pointer.
    let func: Symbol<unsafe extern "C" fn() -> NativeTS> = lib
      .get(name.as_bytes())
      .map_err(DynamicLangError::ReadSymbol)?;
    let lang = func();
    // ATTENTIOIN: dragon ahead
    // TODO: make a struct to hold lib reference
    std::mem::forget(lib);
    //  end of mem safety critical code
    lang
  };
  let version = lang.version();
  if !(MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&version) {
    Err(DynamicLangError::IncompatibleVersion(version))
  } else {
    Ok(lang.into())
  }
}

static mut DYNAMIC_LANG: Option<HashMap<String, DynamicLang>> = None;

#[derive(Default)]
pub struct Registration {
  path: PathBuf,
  name: String,
  meta_var_char: Option<char>,
  expando_char: Option<char>,
  extensions: Vec<String>,
}

impl DynamicLang {
  /// # Safety
  /// the register function should be called exactly once before use.
  /// It relies on a global mut static variable to be initialized.
  pub unsafe fn register(regs: Vec<Registration>) -> Result<(), DynamicLangError> {
    let mut langs = HashMap::new();
    for reg in regs {
      Self::register_one(reg, &mut langs)?;
    }
    DYNAMIC_LANG.replace(langs);
    Ok(())
  }

  fn register_one(
    reg: Registration,
    langs: &mut HashMap<String, DynamicLang>,
  ) -> Result<(), DynamicLangError> {
    let lang = load_ts_language(reg.path, reg.name)?;
    let meta_var_char = reg.meta_var_char.unwrap_or('$');
    let expando_char = reg.expando_char.unwrap_or(meta_var_char);
    let dynamic = DynamicLang {
      lang,
      meta_var_char,
      expando_char,
    };
    for ext in reg.extensions {
      langs.insert(ext, dynamic.clone());
    }
    Ok(())
  }
}

impl Language for DynamicLang {
  /// tree sitter language to parse the source
  fn get_ts_language(&self) -> TSLanguage {
    self.lang.clone()
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
    self.meta_var_char
  }

  /// Some language does not accept $ as the leading char for identifiers.
  /// We need to change $ to other char at run-time to make parser happy, thus the name expando.
  /// By default this is the same as meta_var char so replacement is done at runtime.
  #[inline]
  fn expando_char(&self) -> char {
    self.expando_char
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_load_parser() {
    let lang = load_ts_language(
      "../../benches/fixtures/json.so".into(),
      "tree_sitter_json".into(),
    )
    .unwrap();
    let dl = DynamicLang {
      lang,
      meta_var_char: '$',
      expando_char: '$',
    };
    let sg = dl.ast_grep("{\"a\": 123}");
    assert_eq!(
      sg.root().to_sexp(),
      "(document (object (pair key: (string (string_content)) value: (number))))"
    );
  }
}
