mod doc;
mod sg_node;
mod ts_types;
mod wasm_lang;

pub use sg_node::{SgNode, SgRoot};
pub use wasm_lang::WasmLangInfo;

use doc::{WasmConfig, WasmDoc};
use wasm_lang::WasmLang;

use ast_grep_core::matcher::DumpPattern;
use ast_grep_core::{AstGrep, Language, MatchStrictness, Pattern};
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

/// Dump a pattern's internal structure for inspection.
/// `selector` is an optional kind name for contextual patterns.
/// `strictness` is one of: "cst", "smart", "ast", "relaxed", "signature", "template".
/// Returns a tree structure showing how ast-grep parses the pattern.
#[wasm_bindgen(js_name = dumpPattern)]
pub fn dump_pattern(
  lang: String,
  pattern_str: String,
  selector: Option<String>,
  strictness: Option<String>,
) -> Result<JsValue, JsError> {
  let lang: WasmLang = lang
    .parse()
    .map_err(|e: wasm_lang::NotSupport| JsError::new(&e.to_string()))?;
  let mut pat = if let Some(sel) = &selector {
    Pattern::contextual(&pattern_str, sel, lang).map_err(|e| JsError::new(&e.to_string()))?
  } else {
    Pattern::try_new(&pattern_str, lang).map_err(|e| JsError::new(&e.to_string()))?
  };
  if let Some(s) = &strictness {
    let strict: MatchStrictness = s.parse().map_err(|e: &str| JsError::new(e))?;
    pat = pat.with_strictness(strict);
  }
  let ts_lang = lang.get_ts_language();
  let kind_map = move |kind_id: u16| -> Option<std::borrow::Cow<'static, str>> {
    let name: String = ts_lang.node_kind_for_id(kind_id)?;
    Some(std::borrow::Cow::Owned(name))
  };
  let dumped = pat
    .dump(&kind_map)
    .ok_or_else(|| JsError::new("Pattern has no root node"))?;
  let js_val = dump_pattern_to_js(&dumped)?;
  Ok(js_val)
}

fn dump_pattern_to_js(node: &DumpPattern) -> Result<JsValue, JsError> {
  let obj = js_sys::Object::new();
  js_sys::Reflect::set(
    &obj,
    &"isMetaVar".into(),
    &JsValue::from_bool(node.is_meta_var),
  )
  .unwrap();
  js_sys::Reflect::set(
    &obj,
    &"kind".into(),
    &match &node.kind {
      Some(k) => JsValue::from_str(k),
      None => JsValue::NULL,
    },
  )
  .unwrap();
  js_sys::Reflect::set(&obj, &"text".into(), &JsValue::from_str(&node.text)).unwrap();
  let children = js_sys::Array::new();
  for child in &node.children {
    children.push(&dump_pattern_to_js(child)?);
  }
  js_sys::Reflect::set(&obj, &"children".into(), &children).unwrap();
  Ok(obj.into())
}
