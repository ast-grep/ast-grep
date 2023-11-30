use super::SupportLang;
use ignore::types::{Types, TypesBuilder};
use std::collections::HashMap;

// both use vec since lang will be small
static mut LANG_GLOBS: Vec<(SupportLang, Types)> = vec![];

pub type LanguageGlobs = HashMap<SupportLang, Vec<String>>;

pub unsafe fn register(regs: LanguageGlobs) {
  debug_assert!(LANG_GLOBS.is_empty());
  let mut lang_globs = vec![];
  for (lang, globs) in regs {
    let types = build_types(lang, globs);
    lang_globs.push((lang, types));
  }
  _ = std::mem::replace(&mut LANG_GLOBS, lang_globs);
}

fn build_types(lang: SupportLang, globs: Vec<String>) -> Types {
  let mut builder = TypesBuilder::new();
  let name = lang.to_string();
  for glob in globs {
    builder
      .add(&name, &glob)
      .expect("file pattern must compile");
  }
  builder.select(&name);
  builder.build().expect("file type must be valid")
}

fn add_types(builder: &mut TypesBuilder, types: &Types) {
  for def in types.definitions() {
    let name = def.name();
    for glob in def.globs() {
      builder.add(name, glob);
    }
  }
}

pub fn merge_types(type1: &Types, type2: &Types) -> Types {
  let mut builder = TypesBuilder::new();
  add_types(&mut builder, type1);
  add_types(&mut builder, type2);
  builder.build().expect("file type must be valid")
}
