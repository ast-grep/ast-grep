use crate::FrontEndLanguage;

use ignore::types::TypesBuilder;
use ignore::{WalkBuilder, WalkParallel};
use napi::anyhow::anyhow;
use napi::bindgen_prelude::Result;

pub fn build_files(paths: Vec<String>) -> Result<WalkParallel> {
  if paths.is_empty() {
    return Err(anyhow!("paths cannot be empty.").into());
  }
  let types = TypesBuilder::new()
    .add_defaults()
    .select("css")
    .select("html")
    .select("js")
    .select("ts")
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

pub fn select_custom<'b>(
  builder: &'b mut TypesBuilder,
  file_type: &str,
  default_suffix_list: &[&str],
  custom_suffix_list: &[&str],
) -> &'b mut TypesBuilder {
  let mut suffix_list = default_suffix_list.to_vec();
  suffix_list.extend_from_slice(custom_suffix_list);
  for suffix in suffix_list {
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
  let types = types.add_defaults();

  let custom_file_type = language_globs.unwrap_or_else(Vec::new);
  let custom_file_type: Vec<&str> = custom_file_type.iter().map(|s| s.as_str()).collect();
  let types = match lang {
    FrontEndLanguage::TypeScript => select_custom(
      types,
      "myts",
      &["*.ts", "*.mts", "*.cts"],
      &custom_file_type,
    ),
    FrontEndLanguage::Tsx => select_custom(
      types,
      "mytsx",
      &["*.tsx", "*.mtsx", "*.ctsx"],
      &custom_file_type,
    ),
    FrontEndLanguage::Css => select_custom(types, "css", &["*.css", "*.scss"], &custom_file_type),
    FrontEndLanguage::Html => select_custom(
      types,
      "html",
      &["*.html", "*.htm", "*.xhtml"],
      &custom_file_type,
    ),
    FrontEndLanguage::JavaScript => select_custom(
      types,
      "js",
      &["*.cjs", "*.js", "*.mjs", "*.jsx"],
      &custom_file_type,
    ),
  }
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
