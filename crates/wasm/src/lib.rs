mod doc;
mod sg_node;
mod ts_types;
mod wasm_lang;

pub use sg_node::{SgNode, SgRoot};
pub use wasm_lang::WasmLangInfo;

use doc::{WasmConfig, WasmDoc};
use wasm_lang::WasmLang;

use ast_grep_core::{AstGrep, Language};
use std::collections::HashMap;
use ts_types::TreeSitter;
use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Initialize the tree-sitter WASM runtime.
/// Must be called before any other function.
#[wasm_bindgen(js_name = initializeTreeSitter)]
pub async fn initialize_tree_sitter() -> Result<(), JsError> {
  TreeSitter::init().await
}

/// Register dynamic languages for parsing.
/// `langs` is a Map of language name to its registration config
/// (with `libraryPath` and optional `expandoChar`).
/// Can be called multiple times; existing languages are updated.
#[wasm_bindgen(js_name = registerDynamicLanguage)]
pub async fn register_dynamic_language(langs: JsValue) -> Result<(), JsError> {
  let langs: HashMap<String, WasmLangInfo> =
    serde_wasm_bindgen::from_value(langs).map_err(|e| JsError::new(&e.to_string()))?;
  WasmLang::register(langs).await
}

/// Parse a string to an ast-grep instance.
#[wasm_bindgen]
pub fn parse(lang: String, src: String) -> Result<SgRoot, JsError> {
  let lang: WasmLang = lang
    .parse()
    .map_err(|e: wasm_lang::NotSupport| JsError::new(&e.to_string()))?;
  let doc = WasmDoc::try_new(src, lang)?;
  Ok(SgRoot::new(AstGrep::doc(doc), "anonymous".into()))
}

/// Get the `kind` number from its string name.
#[wasm_bindgen]
pub fn kind(lang: String, kind_name: String) -> Result<u16, JsError> {
  let lang: WasmLang = lang
    .parse()
    .map_err(|e: wasm_lang::NotSupport| JsError::new(&e.to_string()))?;
  Ok(lang.kind_to_id(&kind_name))
}

/// Compile a string to ast-grep Pattern config.
#[wasm_bindgen]
pub fn pattern(lang: String, pattern_str: String) -> Result<JsValue, JsError> {
  let config = WasmConfig {
    rule: serde_json::json!({ "pattern": pattern_str }),
    constraints: None,
    language: Some(lang),
    utils: None,
    transform: None,
  };
  serde_wasm_bindgen::to_value(&config).map_err(|e| JsError::new(&e.to_string()))
}
