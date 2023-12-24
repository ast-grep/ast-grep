use crate::FrontEndLanguage;

use ignore::types::TypesBuilder;
use ignore::{WalkBuilder, WalkParallel};
use napi::anyhow::anyhow;
use napi::bindgen_prelude::Result;

const fn file_patterns(lang: &FrontEndLanguage) -> (&str, &[&str]) {
  match lang {
    FrontEndLanguage::TypeScript => ("myts", &["*.ts", "*.mts", "*.cts"]),
    FrontEndLanguage::Tsx => ("mytsx", &["*.tsx", "*.mtsx", "*.ctsx"]),
    FrontEndLanguage::Css => ("mycss", &["*.css", "*.scss"]),
    FrontEndLanguage::Html => ("myhtml", &["*.html", "*.htm", "*.xhtml"]),
    FrontEndLanguage::JavaScript => ("myjs", &["*.cjs", "*.js", "*.mjs", "*.jsx"]),
  }
}

pub fn build_files(paths: Vec<String>) -> Result<WalkParallel> {
  if paths.is_empty() {
    return Err(anyhow!("paths cannot be empty.").into());
  }
  let mut types = TypesBuilder::new();
  for lang in FrontEndLanguage::all_langs() {
    let (type_name, default_types) = file_patterns(lang);
    select_custom(&mut types, type_name, default_types, &vec![]);
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
