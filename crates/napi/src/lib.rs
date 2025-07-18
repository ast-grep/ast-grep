#![cfg(not(feature = "napi-noop-in-unit-test"))]

mod doc;
mod find_files;
mod napi_lang;
mod sg_node;

use ast_grep_core::{AstGrep, Language};
use ast_grep_language::SupportLang;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use napi_lang::register_dynamic_language as register_dynamic_language_impl;

use doc::{JsDoc, NapiConfig};
use find_files::{find_in_files_impl, FindConfig, FindInFiles, ParseAsync};
use napi_lang::NapiLang;
use sg_node::SgRoot;

pub use find_files::parse_files;

macro_rules! impl_lang_mod {
  ($name: ident, $lang: ident) => {
    #[napi]
    pub mod $name {
      use super::*;

      #[napi]
      pub fn parse(src: String) -> SgRoot {
        parse_with_lang(SupportLang::$lang.to_string(), src).expect("parse failed")
      }

      #[napi]
      pub fn parse_async(src: String) -> Result<AsyncTask<ParseAsync>> {
        parse_async_with_lang(SupportLang::$lang.to_string(), src)
      }
      #[napi]
      pub fn kind(kind_name: String) -> Result<u16> {
        kind_with_lang(SupportLang::$lang.to_string(), kind_name)
      }
      #[napi]
      pub fn pattern(pattern: String) -> NapiConfig {
        pattern_with_lang(SupportLang::$lang.to_string(), pattern)
      }
      #[napi]
      pub fn find_in_files(
        config: FindConfig,
        callback: Function,
      ) -> Result<AsyncTask<FindInFiles>> {
        find_in_files_impl(SupportLang::$lang.into(), config, callback)
      }
    }
  };
}

// for name conflict in mod
use kind as kind_with_lang;
use parse as parse_with_lang;
use parse_async as parse_async_with_lang;
use pattern as pattern_with_lang;
impl_lang_mod!(html, Html);
impl_lang_mod!(js, JavaScript);
impl_lang_mod!(jsx, JavaScript);
impl_lang_mod!(ts, TypeScript);
impl_lang_mod!(tsx, Tsx);
impl_lang_mod!(css, Css);

/// Parse a string to an ast-grep instance
#[napi]
pub fn parse(lang: String, src: String) -> Result<SgRoot> {
  let doc = JsDoc::try_new(src, lang.parse()?)?;
  Ok(SgRoot(AstGrep::doc(doc), "anonymous".into()))
}

/// Parse a string to an ast-grep instance asynchronously in threads.
/// It utilize multiple CPU cores when **concurrent processing sources**.
/// However, spawning excessive many threads may backfire.
/// Please refer to libuv doc, nodejs' underlying runtime
/// for its default behavior and performance tuning tricks.
#[napi]
pub fn parse_async(lang: String, src: String) -> Result<AsyncTask<ParseAsync>> {
  let lang = lang.parse()?;
  Ok(AsyncTask::new(ParseAsync { src, lang }))
}

/// Get the `kind` number from its string name.
#[napi]
pub fn kind(lang: String, kind_name: String) -> Result<u16> {
  let lang: NapiLang = lang.parse()?;
  let kind = lang.kind_to_id(&kind_name);
  Ok(kind)
}

/// Compile a string to ast-grep Pattern.
#[napi]
pub fn pattern(lang: String, pattern: String) -> NapiConfig {
  NapiConfig {
    rule: serde_json::json!({
      "pattern": pattern,
    }),
    constraints: None,
    language: Some(lang),
    utils: None,
    transform: None,
  }
}

/// Discover and parse multiple files in Rust.
/// `lang` specifies the language.
/// `config` specifies the file path and matcher.
/// `callback` will receive matching nodes found in a file.
#[napi]
pub fn find_in_files(
  lang: String,
  config: FindConfig,
  callback: Function,
) -> Result<AsyncTask<FindInFiles>> {
  let lang: NapiLang = lang.parse()?;
  find_in_files_impl(lang, config, callback)
}

/// Register a dynamic language to ast-grep.
/// `langs` is a Map of language name to its CustomLanguage registration.
#[napi]
pub fn register_dynamic_language(langs: serde_json::Value) -> Result<()> {
  let langs = serde_json::from_value(langs)?;
  register_dynamic_language_impl(langs)
}
