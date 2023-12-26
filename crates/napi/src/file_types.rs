use crate::FrontEndLanguage;

use ignore::types::{Types, TypesBuilder};
use ignore::{WalkBuilder, WalkParallel};
use napi::anyhow::anyhow;
use napi::bindgen_prelude::Result;

use std::collections::HashMap;
use std::path::Path;

pub enum LangOption {
  /// Used when language is inferred from file path
  /// e.g. in parse_files
  Inferred(Vec<(FrontEndLanguage, Types)>),
  /// Used when language is specified
  /// e.g. in frontend_lang.find_in_files
  Specified(FrontEndLanguage),
}

impl LangOption {
  pub fn get_lang(&self, path: &Path) -> Option<FrontEndLanguage> {
    use LangOption::*;
    match self {
      Specified(lang) => Some(*lang),
      Inferred(pairs) => pairs
        .iter()
        .find_map(|(lang, types)| types.matched(path, false).is_whitelist().then(|| *lang)),
    }
  }
  pub fn infer(language_globs: &HashMap<FrontEndLanguage, Vec<String>>) -> Self {
    let mut types = vec![];
    let empty = vec![];
    for lang in FrontEndLanguage::all_langs() {
      let (tpe, list) = file_patterns(lang);
      let mut builder = TypesBuilder::new();
      for pattern in list {
        builder.add(tpe, pattern).expect("should build");
      }
      for pattern in language_globs.get(lang).unwrap_or(&empty) {
        builder.add(tpe, pattern).expect("should build");
      }
      builder.select(tpe);
      types.push((*lang, builder.build().unwrap()));
    }
    Self::Inferred(types)
  }
}

const fn file_patterns(lang: &FrontEndLanguage) -> (&str, &[&str]) {
  match lang {
    FrontEndLanguage::TypeScript => ("myts", &["*.ts", "*.mts", "*.cts"]),
    FrontEndLanguage::Tsx => ("mytsx", &["*.tsx", "*.mtsx", "*.ctsx"]),
    FrontEndLanguage::Css => ("mycss", &["*.css", "*.scss"]),
    FrontEndLanguage::Html => ("myhtml", &["*.html", "*.htm", "*.xhtml"]),
    FrontEndLanguage::JavaScript => ("myjs", &["*.cjs", "*.js", "*.mjs", "*.jsx"]),
  }
}

pub fn build_files(
  paths: Vec<String>,
  language_globs: &HashMap<FrontEndLanguage, Vec<String>>,
) -> Result<WalkParallel> {
  if paths.is_empty() {
    return Err(anyhow!("paths cannot be empty.").into());
  }
  let mut types = TypesBuilder::new();
  let empty = vec![];
  for lang in FrontEndLanguage::all_langs() {
    let (type_name, default_types) = file_patterns(lang);
    let custom = language_globs.get(lang).unwrap_or(&empty);
    select_custom(&mut types, type_name, default_types, custom);
  }
  let types = types.build().unwrap();
  let mut paths = paths.into_iter();
  let mut builder = WalkBuilder::new(paths.next().unwrap());
  for path in paths {
    builder.add(path);
  }
  let walk = builder.types(types).build_parallel();
  Ok(walk)
}

fn select_custom<'b>(
  builder: &'b mut TypesBuilder,
  file_type: &str,
  default_suffix_list: &[&str],
  custom_suffix_list: &[String],
) -> &'b mut TypesBuilder {
  for suffix in default_suffix_list {
    builder
      .add(file_type, suffix)
      .expect("file pattern must compile");
  }
  for suffix in custom_suffix_list {
    builder
      .add(file_type, suffix)
      .expect("file pattern must compile");
  }
  builder.select(file_type)
}

pub fn find_files_with_lang(
  paths: Vec<String>,
  lang: &FrontEndLanguage,
  language_globs: Option<Vec<String>>,
) -> Result<WalkParallel> {
  if paths.is_empty() {
    return Err(anyhow!("paths cannot be empty.").into());
  }

  let mut types = TypesBuilder::new();
  let custom_file_type = language_globs.unwrap_or_else(Vec::new);
  let (type_name, default_types) = file_patterns(lang);
  let types = select_custom(&mut types, type_name, default_types, &custom_file_type)
    .build()
    .unwrap();
  let mut paths = paths.into_iter();
  let mut builder = WalkBuilder::new(paths.next().unwrap());
  for path in paths {
    builder.add(path);
  }
  let walk = builder.types(types).build_parallel();
  Ok(walk)
}
