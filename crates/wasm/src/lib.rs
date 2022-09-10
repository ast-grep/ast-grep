mod utils;

use ast_grep_config::{deserialize_rule, SerializableRule};
use ast_grep_core::language::Language;

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tree_sitter as ts;

use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Serialize, Deserialize)]
pub struct MatchResult {
  pub start: usize,
  pub end: usize,
}

static INSTANCE: OnceCell<ts::Language> = OnceCell::new();

#[wasm_bindgen]
pub async fn setup_parser(parser_path: String) -> Result<(), JsError> {
  ts::TreeSitter::init().await?;
  let mut parser = ts::Parser::new()?;
  let lang = get_lang(parser_path).await?;
  parser.set_language(&lang)?;
  INSTANCE
    .set(lang)
    .expect_throw("set current language error");
  Ok(())
}

#[wasm_bindgen]
pub async fn find_nodes(src: String, config: JsValue) -> Result<String, JsError> {
  let config: SerializableRule = config.into_serde()?;
  let lang = INSTANCE.get().expect_throw("current language is not set");
  let root = lang.ast_grep(src);
  let matcher = deserialize_rule(config, lang.clone())?;
  let ret: Vec<_> = root
    .root()
    .find_all(matcher)
    .map(|n| {
      let start = n.start_pos();
      let end = n.end_pos();
      vec![start.0, start.1, end.0, end.1]
    })
    .collect();
  Ok(format!("{:?}", ret))
}

#[cfg(target_arch = "wasm32")]
async fn get_lang(parser_path: String) -> Result<ts::Language, JsError> {
  let lang = web_tree_sitter_sys::Language::load_path(&parser_path)
    .await
    .map_err(ts::LanguageError::from)?;
  Ok(ts::Language::from(lang))
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_lang(_path: String) -> Result<ts::Language, JsError> {
  unreachable!()
}
