use super::SgLang;
use ignore::types::{Types, TypesBuilder};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

// both use vec since lang will be small
static mut LANG_GLOBS: Vec<(SgLang, Types)> = vec![];

pub type LanguageGlobs = HashMap<String, Vec<String>>;

pub unsafe fn register(regs: LanguageGlobs) {
  debug_assert!(LANG_GLOBS.is_empty());
  let mut lang_globs = vec![];
  for (lang, globs) in regs {
    let types = build_types(&lang, globs);
    let lang = SgLang::from_str(&lang).expect("should work");
    lang_globs.push((lang, types));
  }
  _ = std::mem::replace(&mut LANG_GLOBS, lang_globs);
}

fn build_types(lang: &str, globs: Vec<String>) -> Types {
  let mut builder = TypesBuilder::new();
  for glob in globs {
    builder.add(lang, &glob).expect("file pattern must compile");
  }
  builder.select(lang);
  builder.build().expect("file type must be valid")
}

fn add_types(builder: &mut TypesBuilder, types: &Types) {
  for def in types.definitions() {
    let name = def.name();
    for glob in def.globs() {
      builder.add(name, glob).expect("file type must be valid");
    }
  }
}

pub fn merge_types(type1: Types, type2: Option<&Types>) -> Types {
  let Some(type2) = type2 else {
    return type1;
  };
  let mut builder = TypesBuilder::new();
  add_types(&mut builder, &type1);
  add_types(&mut builder, type2);
  builder.build().expect("file type must be valid")
}

pub fn get_types(lang: &SgLang) -> Option<&Types> {
  for (l, types) in unsafe { &LANG_GLOBS } {
    if l == lang {
      return Some(types);
    }
  }
  None
}

pub fn from_path(p: &Path) -> Option<SgLang> {
  for (lang, types) in unsafe { &LANG_GLOBS } {
    if types.matched(p, false).is_whitelist() {
      return Some(*lang);
    }
  }
  None
}
