#![cfg(not(feature = "napi-noop-in-unit-test"))]

mod doc;
mod find_files;
mod napi_lang;
mod sg_node;

use ast_grep_core::language::Language;
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
use napi::bindgen_prelude::*;
use napi_derive::napi;

use doc::{JsDoc, NapiConfig};
use find_files::{find_in_files_impl, FindConfig, FindInFiles, ParseAsync};
use napi_lang::Lang;
use sg_node::SgRoot;

pub use find_files::parse_files;

macro_rules! impl_lang_mod {
    ($name: ident, $lang: ident) =>  {
      #[napi]
      pub mod $name {
        use super::*;
        use super::Lang::*;

        /// Parse a string to an ast-grep instance
        #[napi]
        pub fn parse(src: String) -> SgRoot {
          parse_with_lang($lang, src)
        }

        /// Parse a string to an ast-grep instance asynchronously in threads.
        /// It utilize multiple CPU cores when **concurrent processing sources**.
        /// However, spawning excessive many threads may backfire.
        /// Please refer to libuv doc, nodejs' underlying runtime
        /// for its default behavior and performance tuning tricks.
        #[napi(ts_return_type = "Promise<SgRoot>")]
        pub fn parse_async(src: String) -> AsyncTask<ParseAsync> {
          parse_async_with_lang($lang, src)
        }
        /// Get the `kind` number from its string name.
        #[napi]
        pub fn kind(kind_name: String) -> u16 {
          kind_with_lang($lang, kind_name)
        }
        /// Compile a string to ast-grep Pattern.
        #[napi]
        pub fn pattern(pattern: String) -> NapiConfig {
          pattern_with_lang($lang, pattern)
        }

        /// Discover and parse multiple files in Rust.
        /// `config` specifies the file path and matcher.
        /// `callback` will receive matching nodes found in a file.
        #[napi(
          ts_args_type = "config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void",
          ts_return_type = "Promise<number>"
        )]
        pub fn find_in_files(config: FindConfig, callback: JsFunction) -> Result<AsyncTask<FindInFiles>> {
          find_in_files_impl($lang, config, callback)
        }
      }
    }
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
pub fn parse(lang: Lang, src: String) -> SgRoot {
  let doc = JsDoc::new(src, lang.into());
  SgRoot(AstGrep::doc(doc), "anonymous".into())
}

/// Parse a string to an ast-grep instance asynchronously in threads.
/// It utilize multiple CPU cores when **concurrent processing sources**.
/// However, spawning excessive many threads may backfire.
/// Please refer to libuv doc, nodejs' underlying runtime
/// for its default behavior and performance tuning tricks.
#[napi(ts_return_type = "Promise<SgRoot>")]
pub fn parse_async(lang: Lang, src: String) -> AsyncTask<ParseAsync> {
  AsyncTask::new(ParseAsync { src, lang })
}

/// Get the `kind` number from its string name.
#[napi]
pub fn kind(lang: Lang, kind_name: String) -> u16 {
  let lang: SupportLang = lang.into();
  lang
    .get_ts_language()
    .id_for_node_kind(&kind_name, /* named */ true)
}

/// Compile a string to ast-grep Pattern.
#[napi]
pub fn pattern(lang: Lang, pattern: String) -> NapiConfig {
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
#[napi(
  ts_args_type = "lang: Lang, config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void",
  ts_return_type = "Promise<number>"
)]
pub fn find_in_files(
  lang: Lang,
  config: FindConfig,
  callback: JsFunction,
) -> Result<AsyncTask<FindInFiles>> {
  find_in_files_impl(lang, config, callback)
}
