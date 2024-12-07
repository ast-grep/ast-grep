use super::SgLang;
use ignore::types::{Types, TypesBuilder};
use std::collections::HashMap;
use std::path::Path;
use std::ptr::{addr_of, addr_of_mut};
use std::str::FromStr;

use crate::utils::ErrorContext as EC;
use anyhow::{Context, Result};

// both use vec since lang will be small
static mut LANG_GLOBS: Vec<(SgLang, Types)> = vec![];

pub type LanguageGlobs = HashMap<String, Vec<String>>;

pub unsafe fn register(regs: LanguageGlobs) -> Result<()> {
  debug_assert! {
    (*addr_of!(LANG_GLOBS)).is_empty()
  };
  let lang_globs = register_impl(regs)?;
  _ = std::mem::replace(&mut *addr_of_mut!(LANG_GLOBS), lang_globs);
  Ok(())
}

fn register_impl(regs: LanguageGlobs) -> Result<Vec<(SgLang, Types)>> {
  let mut lang_globs = vec![];
  for (lang, globs) in regs {
    let lang = SgLang::from_str(&lang).with_context(|| EC::UnrecognizableLanguage(lang))?;
    // Note: we have to use lang.to_string() for normalized language name
    // TODO: add test
    let lang_name = lang.to_string();
    let types = build_types(&lang_name, globs)?;
    lang_globs.push((lang, types));
  }
  Ok(lang_globs)
}

fn build_types(lang: &str, globs: Vec<String>) -> Result<Types> {
  let mut builder = TypesBuilder::new();
  for glob in globs {
    // builder add will only trigger error when lang name is `all`
    builder
      .add(lang, &glob)
      .with_context(|| EC::UnrecognizableLanguage(lang.into()))?;
  }
  builder.select(lang);
  builder.build().context(EC::ParseConfiguration)
}

fn add_types(builder: &mut TypesBuilder, types: &Types) {
  for def in types.definitions() {
    let name = def.name();
    for glob in def.globs() {
      builder.add(name, glob).expect(name);
    }
  }
}

fn get_types(lang: &SgLang) -> Option<&Types> {
  for (l, types) in unsafe { &*addr_of!(LANG_GLOBS) } {
    if l == lang {
      return Some(types);
    }
  }
  None
}

pub fn merge_types(types_vec: impl Iterator<Item = Types>) -> Types {
  let mut builder = TypesBuilder::new();
  for types in types_vec {
    for def in types.definitions() {
      let name = def.name();
      for glob in def.globs() {
        builder.add(name, glob).expect(name);
      }
      builder.select(name);
    }
  }
  builder.build().expect("file types must be valid")
}

pub fn merge_globs(lang: &SgLang, type1: Types) -> Types {
  let Some(type2) = get_types(lang) else {
    return type1;
  };
  let mut builder = TypesBuilder::new();
  add_types(&mut builder, &type1);
  add_types(&mut builder, type2);
  builder.select(&lang.to_string());
  builder.build().expect("file type must be valid")
}

pub fn from_path(p: &Path) -> Option<SgLang> {
  for (lang, types) in unsafe { &*addr_of!(LANG_GLOBS) } {
    if types.matched(p, false).is_whitelist() {
      return Some(*lang);
    }
  }
  None
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_language::SupportLang;
  use serde_yaml::from_str;

  const YAML: &str = r"
js: ['.eslintrc']
html: ['*.vue', '*.svelte']";

  fn get_globs() -> LanguageGlobs {
    from_str(YAML).expect("should parse")
  }
  #[test]
  fn test_parse_globs() {
    let globs = get_globs();
    assert_eq!(globs["js"], &[".eslintrc"]);
    assert_eq!(globs["html"], &["*.vue", "*.svelte"]);
  }

  #[test]
  fn test_register() -> Result<()> {
    let globs = get_globs();
    let lang_globs = register_impl(globs)?;
    assert_eq!(lang_globs.len(), 2);
    Ok(())
  }

  #[test]
  fn test_invalid_language() {
    let mut globs = get_globs();
    globs.insert("php-exp".into(), vec!["bestlang".into()]);
    let ret = register_impl(globs);
    let err = ret.expect_err("should wrong");
    assert!(matches!(
      err.downcast::<EC>(),
      Ok(EC::UnrecognizableLanguage(_))
    ));
  }

  #[test]
  fn test_merge_types() {
    let lang: SgLang = SupportLang::Rust.into();
    let default_types = lang.file_types();
    let rust_types = merge_globs(&lang, default_types);
    assert!(rust_types.matched("a.php", false).is_ignore());
    assert!(rust_types.matched("a.rs", false).is_whitelist());
  }

  #[test]
  fn test_merge_with_globs() -> Result<()> {
    let globs = get_globs();
    unsafe {
      // cleanup
      std::mem::take(&mut *addr_of_mut!(LANG_GLOBS));
      register(globs)?;
      assert_eq!((*addr_of!(LANG_GLOBS)).len(), 2);
    }
    let lang: SgLang = SupportLang::Html.into();
    let default_types = lang.file_types();
    let html_types = merge_globs(&lang, default_types);
    assert!(html_types.matched("a.php", false).is_ignore());
    assert!(html_types.matched("a.html", false).is_whitelist());
    assert!(html_types.matched("a.vue", false).is_whitelist());
    assert!(html_types.matched("a.svelte", false).is_whitelist());
    Ok(())
  }
}
