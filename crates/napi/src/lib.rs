#![cfg(not(feature = "napi-noop-in-unit-test"))]

mod doc;
mod fe_lang;
mod find_files;
mod sg_node;

use ast_grep_core::language::Language;
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
use napi::bindgen_prelude::*;
use napi_derive::napi;

use doc::{JsDoc, NapiConfig};
use fe_lang::FrontEndLanguage;
use find_files::{find_in_files_impl, FindConfig, FindInFiles, ParseAsync};
use sg_node::SgRoot;

pub use find_files::parse_files;

macro_rules! impl_lang_mod {
    ($name: ident, $lang: ident) =>  {
      #[napi]
      pub mod $name {
        use super::*;
        use super::FrontEndLanguage::*;

        /// Parse a string to an ast-grep instance
        #[napi]
        pub fn parse(src: String) -> SgRoot {
          let doc = JsDoc::new(src, ($lang).into());
          SgRoot(AstGrep::doc(doc), "anonymous".into())
        }

        /// Parse a string to an ast-grep instance asynchronously in threads.
        /// It utilize multiple CPU cores when **concurrent processing sources**.
        /// However, spawning excessive many threads may backfire.
        /// Please refer to libuv doc, nodejs' underlying runtime
        /// for its default behavior and performance tuning tricks.
        #[napi(ts_return_type = "Promise<SgRoot>")]
        pub fn parse_async(src: String) -> AsyncTask<ParseAsync> {
          AsyncTask::new(ParseAsync {
            src, lang: $lang,
          })
        }
        /// Get the `kind` number from its string name.
        #[napi]
        pub fn kind(kind_name: String) -> u16 {
          let lang: SupportLang = $lang.into();
          lang.get_ts_language().id_for_node_kind(&kind_name, /* named */ true)
        }
        /// Compile a string to ast-grep Pattern.
        #[napi]
        pub fn pattern(pattern: String) -> NapiConfig {
          NapiConfig {
            rule: serde_json::json!({
              "pattern": pattern,
            }),
            constraints: None,
            language: Some($lang),
            utils: None,
            transform: None,
          }
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

impl_lang_mod!(html, Html);
impl_lang_mod!(js, JavaScript);
impl_lang_mod!(jsx, JavaScript);
impl_lang_mod!(ts, TypeScript);
impl_lang_mod!(tsx, Tsx);
impl_lang_mod!(css, Css);
