use ast_grep_core::language::{Language, TSLanguage};
use ignore::types::{Types, TypesBuilder};
use ignore::{WalkBuilder, WalkParallel};
use napi::anyhow::anyhow;
use napi::anyhow::Error;
use napi::bindgen_prelude::Result;
use napi_derive::napi;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

#[napi]
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum FrontEndLanguage {
  Html,
  JavaScript,
  Tsx,
  Css,
  TypeScript,
}

pub type LanguageGlobs = HashMap<FrontEndLanguage, Vec<String>>;

impl FrontEndLanguage {
  pub const fn all_langs() -> &'static [FrontEndLanguage] {
    use FrontEndLanguage::*;
    &[Html, JavaScript, Tsx, Css, TypeScript]
  }
  pub fn lang_globs(map: HashMap<String, Vec<String>>) -> LanguageGlobs {
    let mut ret = HashMap::new();
    for (name, patterns) in map {
      if let Ok(lang) = FrontEndLanguage::from_str(&name) {
        ret.insert(lang, patterns);
      }
    }
    ret
  }

  pub fn find_files(
    &self,
    paths: Vec<String>,
    language_globs: Option<Vec<String>>,
  ) -> Result<WalkParallel> {
    find_files_with_lang(self, paths, language_globs)
  }
}

impl Language for FrontEndLanguage {
  fn get_ts_language(&self) -> TSLanguage {
    use FrontEndLanguage::*;
    match self {
      Html => tree_sitter_html::language(),
      JavaScript => tree_sitter_javascript::language(),
      TypeScript => tree_sitter_typescript::language_typescript(),
      Css => tree_sitter_css::language(),
      Tsx => tree_sitter_typescript::language_tsx(),
    }
    .into()
  }
  fn expando_char(&self) -> char {
    use FrontEndLanguage::*;
    match self {
      Css => '_',
      _ => '$',
    }
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    use FrontEndLanguage::*;
    match self {
      Css => (),
      _ => return Cow::Borrowed(query),
    }
    // use stack buffer to reduce allocation
    let mut buf = [0; 4];
    let expando = self.expando_char().encode_utf8(&mut buf);
    // TODO: use more precise replacement
    let replaced = query.replace(self.meta_var_char(), expando);
    Cow::Owned(replaced)
  }
}

const fn alias(lang: &FrontEndLanguage) -> &[&str] {
  use FrontEndLanguage::*;
  match lang {
    Css => &["css"],
    Html => &["html"],
    JavaScript => &["javascript", "js", "jsx"],
    TypeScript => &["ts", "typescript"],
    Tsx => &["tsx"],
  }
}

impl FromStr for FrontEndLanguage {
  type Err = Error;
  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    for lang in Self::all_langs() {
      for moniker in alias(lang) {
        if s.eq_ignore_ascii_case(moniker) {
          return Ok(*lang);
        }
      }
    }
    Err(anyhow!(format!("{} is not supported in napi", s.to_string())).into())
  }
}

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
  pub fn infer(language_globs: &LanguageGlobs) -> Self {
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

pub fn build_files(paths: Vec<String>, language_globs: &LanguageGlobs) -> Result<WalkParallel> {
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

fn find_files_with_lang(
  lang: &FrontEndLanguage,
  paths: Vec<String>,
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

#[cfg(test)]
mod test {
  use super::*;

  fn lang_globs() -> HashMap<FrontEndLanguage, Vec<String>> {
    let mut lang = HashMap::new();
    lang.insert("html".into(), vec!["*.vue".into()]);
    FrontEndLanguage::lang_globs(lang)
  }

  #[test]
  fn test_lang_globs() {
    let globs = lang_globs();
    assert!(globs.contains_key(&FrontEndLanguage::Html));
    assert!(!globs.contains_key(&FrontEndLanguage::Tsx));
    assert_eq!(globs[&FrontEndLanguage::Html], vec!["*.vue"]);
  }

  #[test]
  fn test_lang_option() {
    let globs = lang_globs();
    let option = LangOption::infer(&globs);
    let lang = option.get_lang(Path::new("test.vue"));
    assert_eq!(lang, Some(FrontEndLanguage::Html));
    let lang = option.get_lang(Path::new("test.html"));
    assert_eq!(lang, Some(FrontEndLanguage::Html));
    let lang = option.get_lang(Path::new("test.js"));
    assert_eq!(lang, Some(FrontEndLanguage::JavaScript));
    let lang = option.get_lang(Path::new("test.xss"));
    assert_eq!(lang, None);
  }

  #[test]
  fn test_from_str() {
    let lang = FrontEndLanguage::from_str("html");
    assert_eq!(lang.unwrap(), FrontEndLanguage::Html);
    let lang = FrontEndLanguage::from_str("Html");
    assert_eq!(lang.unwrap(), FrontEndLanguage::Html);
    let lang = FrontEndLanguage::from_str("htML");
    assert_eq!(lang.unwrap(), FrontEndLanguage::Html);
    let lang = FrontEndLanguage::from_str("ocaml");
    assert!(lang.is_err());
  }
}
