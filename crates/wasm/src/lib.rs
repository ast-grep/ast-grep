mod utils;

use ast_grep_config::{deserialize_rule, SerializableRule};
use ast_grep_core::language::Language;

use serde::{Deserialize, Serialize};

use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Serialize, Deserialize)]
pub struct MatchResult {
  pub start: usize,
  pub end: usize,
}

#[wasm_bindgen]
pub async fn find_nodes(
  src: String,
  config: JsValue,
  parser_path: String,
) -> Result<String, JsError> {
  tree_sitter::TreeSitter::init().await?;
  let mut parser = tree_sitter::Parser::new()?;
  let lang = get_lang(parser_path).await?;
  parser.set_language(&lang).expect_throw("set lang");
  let config: SerializableRule = config.into_serde()?;
  let root = lang.ast_grep(src);
  let matcher = deserialize_rule(config, lang)?;
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
async fn get_lang(parser_path: String) -> Result<tree_sitter::Language, JsError> {
  let lang = web_tree_sitter_sys::Language::load_path(&parser_path)
    .await
    .map_err(tree_sitter::LanguageError::from)?;
  Ok(tree_sitter::Language::from(lang))
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_lang(_path: String) -> Result<tree_sitter::Language, JsError> {
  unreachable!()
}
