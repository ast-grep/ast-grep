mod doc;
mod sg_node;
mod ts_types;
mod wasm_lang;

pub use sg_node::{SgNode, SgRoot};
pub use wasm_lang::WasmLangInfo;

use doc::{WasmConfig, WasmDoc};
use wasm_lang::WasmLang;

use ast_grep_core::matcher::PatternNode;
use ast_grep_core::{AstGrep, Language, MatchStrictness, Node as CoreNode, Pattern};
use std::collections::HashMap;
use ts_types::TreeSitter;
use wasm_bindgen::prelude::*;

/// Initialize the tree-sitter WASM runtime.
/// Must be called before any other function.
#[wasm_bindgen(js_name = initializeTreeSitter)]
pub async fn initialize_tree_sitter() -> Result<(), JsError> {
  TreeSitter::init().await
}

// Inject custom TypeScript
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
export function registerDynamicLanguage(map: Record<string, {libraryPath: string, expandoChar?: string}>): Promise<void>;
"#;

/// Register dynamic languages for parsing.
/// `langs` is a Map of language name to its registration config
/// (with `libraryPath` and optional `expandoChar`).
/// Can be called multiple times; existing languages are updated.
#[wasm_bindgen(js_name = registerDynamicLanguage, skip_typescript)]
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

// --- Pattern tree types ---

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum PatternKind {
  Terminal,
  MetaVar,
  Internal,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PatternPos {
  line: u32,
  column: u32,
}

impl From<ts_types::Point> for PatternPos {
  fn from(p: ts_types::Point) -> Self {
    PatternPos {
      line: p.row(),
      column: p.column(),
    }
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternTree {
  kind: String,
  start: PatternPos,
  end: PatternPos,
  is_named: bool,
  children: Vec<PatternTree>,
  text: Option<String>,
  pattern: Option<PatternKind>,
}

/// Dump a pattern's internal structure for inspection.
/// `selector` is an optional kind name for contextual patterns.
/// `strictness` is one of: "cst", "smart", "ast", "relaxed", "signature", "template".
/// Returns a tree structure showing how ast-grep parses the pattern, including source positions.
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
  // Pre-process the pattern string so tree-sitter can parse it as valid code.
  // Pattern::try_new also calls pre_process_pattern internally, but we need a
  // separate WasmDoc so we can look up positions from the actual parsed tree.
  let processed = lang.pre_process_pattern(&pattern_str);
  let doc = WasmDoc::try_new(processed.to_string(), lang)?;
  let root = AstGrep::doc(doc);
  let mut pat = if let Some(sel) = &selector {
    Pattern::contextual(&pattern_str, sel, lang).map_err(|e| JsError::new(&e.to_string()))?
  } else {
    Pattern::try_new(&pattern_str, lang).map_err(|e| JsError::new(&e.to_string()))?
  };
  if let Some(s) = &strictness {
    let strict: MatchStrictness = s.parse().map_err(|e: &str| JsError::new(e))?;
    pat = pat.with_strictness(strict);
  }
  let found = root
    .root()
    .find(&pat)
    .ok_or_else(|| JsError::new("Pattern has no root node"))?;
  let tree = dump_pattern_node(found.into(), &pat.node);
  serde_wasm_bindgen::to_value(&tree).map_err(|e| JsError::new(&e.to_string()))
}

fn dump_pattern_node<'r>(node: CoreNode<'r, WasmDoc>, pattern: &PatternNode) -> PatternTree {
  use PatternNode as PN;
  let ts = node.get_inner_node().0;
  let kind = if ts.is_missing() {
    format!("MISSING {}", node.kind())
  } else {
    node.kind().to_string()
  };
  match pattern {
    PN::MetaVar { .. } => {
      let expando = node.lang().expando_char();
      let text = node.text().to_string().replace(expando, "$");
      PatternTree {
        kind,
        start: ts.start_position().into(),
        end: ts.end_position().into(),
        is_named: true,
        children: vec![],
        text: Some(text),
        pattern: Some(PatternKind::MetaVar),
      }
    }
    PN::Terminal { is_named, .. } => PatternTree {
      kind,
      start: ts.start_position().into(),
      end: ts.end_position().into(),
      is_named: *is_named,
      children: vec![],
      text: Some(node.text().into_owned()),
      pattern: Some(PatternKind::Terminal),
    },
    PN::Internal { children, .. } => {
      let children = children
        .iter()
        .zip(node.children())
        .map(|(pn, n)| dump_pattern_node(n, pn))
        .collect();
      PatternTree {
        kind,
        start: ts.start_position().into(),
        end: ts.end_position().into(),
        is_named: true,
        children,
        text: None,
        pattern: Some(PatternKind::Internal),
      }
    }
  }
}
