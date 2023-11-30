use super::SgLang;
use ignore::types::{Types, TypesBuilder};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use crate::error::ErrorContext as EC;
use anyhow::{Context, Result};

// both use vec since lang will be small
static mut LANG_GLOBS: Vec<(SgLang, Types)> = vec![];

pub type LanguageGlobs = HashMap<String, Vec<String>>;

pub unsafe fn register(regs: LanguageGlobs) -> Result<()> {
  debug_assert!(LANG_GLOBS.is_empty());
  let mut lang_globs = vec![];
  for (lang, globs) in regs {
    let lang = SgLang::from_str(&lang).with_context(|| EC::UnrecognizableLanguage(lang))?;
    // Note: we have to use lang.to_string() for normalized language name
    // TODO: add test
    let lang_name = lang.to_string();
    let types = build_types(&lang_name, globs)?;
    lang_globs.push((lang, types));
  }
  _ = std::mem::replace(&mut LANG_GLOBS, lang_globs);
  Ok(())
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
      builder.add(name, glob).expect("file type must be valid");
    }
  }
}

fn get_types(lang: &SgLang) -> Option<&Types> {
  for (l, types) in unsafe { &LANG_GLOBS } {
    if l == lang {
      return Some(types);
    }
  }
  None
}

pub fn merge_types(lang: &SgLang, type1: Types) -> Types {
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
  for (lang, types) in unsafe { &LANG_GLOBS } {
    if types.matched(p, false).is_whitelist() {
      return Some(*lang);
    }
  }
  None
}
