use crate::{DynamicLang, DynamicLangError, Registration};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum LibraryPath {
  Single(PathBuf),
  Platform(HashMap<String, PathBuf>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomLang {
  pub library_path: LibraryPath,
  /// the dylib symbol to load ts-language, default is `tree_sitter_{name}`
  pub language_symbol: Option<String>,
  pub meta_var_char: Option<char>,
  pub expando_char: Option<char>,
  pub extensions: Vec<String>,
}

impl CustomLang {
  pub fn register(base: &Path, langs: HashMap<String, CustomLang>) -> Result<(), DynamicLangError> {
    let registrations: Result<_, _> = langs
      .into_iter()
      .map(|(name, custom)| to_registration(name, custom, base))
      .collect();
    unsafe { DynamicLang::register(registrations?) }
  }
}

fn to_registration(
  name: String,
  custom_lang: CustomLang,
  base: &Path,
) -> Result<Registration, DynamicLangError> {
  let lib_path = match custom_lang.library_path {
    LibraryPath::Single(path) => path,
    LibraryPath::Platform(mut map) => {
      let target = target_triple::TARGET;
      map
        .remove(target)
        .ok_or(DynamicLangError::NotConfigured(target))?
    }
  };
  let path = base.join(lib_path);
  let sym = custom_lang
    .language_symbol
    .unwrap_or_else(|| format!("tree_sitter_{name}"));
  Ok(Registration {
    lang_name: name,
    lib_path: path,
    symbol: sym,
    meta_var_char: custom_lang.meta_var_char,
    expando_char: custom_lang.expando_char,
    extensions: custom_lang.extensions,
  })
}

#[cfg(test)]
mod test {
  use super::*;
  use serde_yaml::from_str;

  #[test]
  fn test_custom_lang() {
    let yaml = r"
libraryPath: a/b/c.so
extensions: [d, e, f]";
    let cus: CustomLang = from_str(yaml).unwrap();
    assert_eq!(cus.language_symbol, None);
    assert_eq!(cus.extensions, vec!["d", "e", "f"]);
  }
  fn is_test_supported() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
      || cfg!(all(target_os = "linux", target_arch = "x86_64"))
  }

  #[test]
  fn test_custom_lang_platform() {
    if !is_test_supported() {
      return;
    }
    let yaml = r"
libraryPath:
  aarch64-apple-darwin: a/b/c.so
  x86_64-unknown-linux-gnu: a/b/c.so
extensions: [d, e, f]";
    let cus: CustomLang = from_str(yaml).unwrap();
    assert_eq!(cus.language_symbol, None);
    assert_eq!(cus.extensions, vec!["d", "e", "f"]);
    let registration = to_registration("test_lang".to_string(), cus, Path::new(".")).unwrap();
    assert_eq!(registration.lang_name, "test_lang");
    assert_eq!(registration.lib_path.to_str(), Some("./a/b/c.so"));
  }

  #[test]
  fn test_unsupport_platform() {
    let yaml = r"
libraryPath:
  impossible-platform: a/b/c.so
extensions: [d, e, f]";
    let cus: CustomLang = from_str(yaml).unwrap();
    let reg = to_registration("test_lang".to_string(), cus, Path::new("."));
    assert!(matches!(
      reg,
      Err(DynamicLangError::NotConfigured(target_triple::TARGET))
    ));
  }
}
