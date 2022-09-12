mod utils;

use ast_grep_config::{
  deserialize_rule, try_deserialize_matchers, RuleWithConstraint, SerializableMetaVarMatcher,
  SerializableRule,
};
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarMatchers;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tree_sitter as ts;
use wasm_bindgen::prelude::*;

use std::sync::Mutex;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Serialize, Deserialize)]
pub struct MatchResult {
  pub start: usize,
  pub end: usize,
}

#[derive(Serialize, Deserialize)]
pub struct WASMConfig {
  pub rule: SerializableRule,
  pub fix: Option<String>,
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
}

static INSTANCE: Mutex<Option<ts::Language>> = Mutex::new(None);

#[wasm_bindgen]
pub async fn setup_parser(parser_path: String) -> Result<(), JsError> {
  ts::TreeSitter::init().await?;
  let mut parser = ts::Parser::new()?;
  let lang = get_lang(parser_path).await?;
  parser.set_language(&lang)?;
  let mut curr_lang = INSTANCE.lock().expect_throw("set language error");
  *curr_lang = Some(lang);
  Ok(())
}

#[wasm_bindgen]
pub async fn find_nodes(src: String, config: JsValue) -> Result<String, JsError> {
  let config: WASMConfig = config.into_serde()?;
  let lang = INSTANCE
    .lock()
    .expect_throw("get language error")
    .clone()
    .expect_throw("current language is not set");
  let root = lang.ast_grep(src);
  let rule = deserialize_rule(config.rule, lang.clone())?;
  let matchers = if let Some(c) = config.constraints {
    try_deserialize_matchers(c, lang).unwrap()
  } else {
    MetaVarMatchers::default()
  };
  let config = RuleWithConstraint { rule, matchers };
  let ret: Vec<_> = root
    .root()
    .find_all(config)
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
