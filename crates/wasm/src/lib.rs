pub mod wasm_libc;

use ast_grep_config::{CombinedScan, RuleConfig};
use ast_grep_core::{AstGrep, Language, Node, StrDoc};
use ast_grep_language::SupportLang;
use config::try_get_rule_config;
use dump_tree::{dump_one_node, dump_pattern as dump_pattern_impl, DumpNode};
use sg_node::SgRoot;
use std::{collections::HashMap, str::FromStr};
use utils::WasmMatch;
use wasm_bindgen::prelude::*;

mod config;
mod dump_tree;
pub mod sg_node;
mod types;
mod utils;

#[wasm_bindgen(typescript_custom_section)]
const ITEXT_STYLE: &'static str = r#"
export function parse<M extends TypesMap>(lang: WasmLang, src: string): SgRoot<M>;
export function parseAsync<M extends TypesMap>(lang: WasmLang, src: string): Promise<SgRoot<M>>;
export function scanFind<M extends TypesMap>(src: string, configs: CompleteRuleConfig<M>[]): Map<string, SgMatch<M>[]>;
export function scanFix<M extends TypesMap>(src: string, configs: CompleteRuleConfig<M>[]): string;
"#;

// We may not need these anymore as we're using the types.d.ts file
// But keeping them for clarity in the Rust code
#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(typescript_type = "SgRoot<M>")]
  pub type ISgRoot;

  #[wasm_bindgen(typescript_type = "Promise<SgRoot<M>>")]
  pub type IPromiseSgRoot;

  #[wasm_bindgen(typescript_type = "CompleteRuleConfig<M>")]
  pub type ICompleteRuleConfig;
}

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// Initialize panic hook for better error messages
fn init_panic_hook() {
  console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = parse, skip_typescript)]
pub fn parse(lang: String, src: String) -> Result<ISgRoot, JsError> {
  init_panic_hook();

  let lang =
    SupportLang::from_str(&lang).map_err(|e| JsError::new(&format!("Language error: {}", e)))?;

  let doc = StrDoc::new(&src, lang);

  Ok(
    JsValue::from(SgRoot {
      inner: AstGrep::doc(doc),
    })
    .into(),
  )
}

#[wasm_bindgen(js_name = parseAsync, skip_typescript)]
pub async fn parse_async(lang: String, src: String) -> Result<IPromiseSgRoot, JsError> {
  init_panic_hook();

  let lang =
    SupportLang::from_str(&lang).map_err(|e| JsError::new(&format!("Language error: {}", e)))?;

  let doc = StrDoc::new(&src, lang);

  Ok(
    JsValue::from(SgRoot {
      inner: AstGrep::doc(doc),
    })
    .into(),
  )
}

#[wasm_bindgen(js_name = kind)]
pub fn kind(lang: String, kind_name: String) -> Result<u16, JsError> {
  init_panic_hook();

  let lang =
    SupportLang::from_str(&lang).map_err(|e| JsError::new(&format!("Language error: {}", e)))?;

  let kind = lang
    .get_ts_language()
    .id_for_node_kind(&kind_name, /* named */ true);

  Ok(kind)
}

// Scan and fix
#[wasm_bindgen(js_name = scanFind, skip_typescript)]
pub fn scan_find(src: String, configs: Vec<JsValue>) -> Result<JsValue, JsError> {
  let mut lang = None;
  let mut rules = vec![];
  for config in configs {
    let finder = try_get_rule_config(config)?;
    let current_lang = finder.language;
    if lang.is_none() {
      lang = Some(current_lang);
    } else if lang.unwrap() != current_lang {
      return Err(JsError::new("Inconsistent languages in configs"));
    }
    rules.push(finder);
  }

  let lang = lang.ok_or_else(|| JsError::new("No language specified in configs"))?;
  let combined = CombinedScan::new(rules.iter().collect());
  let doc = StrDoc::new(&src, lang);
  let root = AstGrep::doc(doc);
  let ret: HashMap<_, _> = combined
    .scan(&root, false)
    .matches
    .into_iter()
    .map(|(rule, matches)| {
      let matches: Vec<_> = matches
        .into_iter()
        .map(|m| WasmMatch::from_match(m, rule))
        .collect();
      (rule.id.clone(), matches)
    })
    .collect();
  let ret = serde_wasm_bindgen::to_value(&ret)?;
  Ok(ret)
}

#[wasm_bindgen(js_name = scanFix, skip_typescript)]
pub fn scan_fix(src: String, configs: Vec<ICompleteRuleConfig>) -> Result<String, JsError> {
  // Extract language from configs
  let mut lang = None;
  let mut rules = vec![];

  for config in configs {
    let finder = try_get_rule_config(config.into())?;
    let current_lang = finder.language;
    if lang.is_none() {
      lang = Some(current_lang);
    } else if lang.unwrap() != current_lang {
      return Err(JsError::new("Inconsistent languages in configs"));
    }

    rules.push(finder);
  }

  let lang = lang.ok_or_else(|| JsError::new("No language specified in configs"))?;
  let rules_ref: Vec<&RuleConfig<_>> = rules.iter().collect();
  let combined = CombinedScan::new(rules_ref);
  let doc = StrDoc::new(&src, lang);
  let root = AstGrep::doc(doc);
  let matches = combined.scan(&root, true);
  let diffs = matches.diffs;

  if diffs.is_empty() {
    return Ok(src);
  }
  let mut start = 0;
  let src: Vec<_> = src.chars().collect();

  let mut new_content = Vec::<char>::new();
  for (rule, nm) in diffs {
    let range = nm.range();
    if start > range.start {
      continue;
    }
    let fixer = rule
      .get_fixer()?
      .expect("rule returned by diff must have fixer");
    let edit = nm.make_edit(&rule.matcher, &fixer);
    new_content.extend(&src[start..edit.position]);
    // Convert the inserted_text bytes to chars before extending
    let inserted_chars: Vec<char> = edit.inserted_text.iter().map(|&b| b as char).collect();
    new_content.extend(inserted_chars);
    start = edit.position + edit.deleted_length;
  }
  // add trailing statements
  new_content.extend(&src[start..]);
  Ok(new_content.into_iter().collect())
}

// Dump AST nodes and patterns
fn convert_to_debug_node(n: Node<StrDoc<SupportLang>>) -> DumpNode {
  let mut cursor = n.get_ts_node().walk();
  let mut target = vec![];
  dump_one_node(&mut cursor, &mut target);
  target.pop().expect_throw("found empty node")
}

#[wasm_bindgen(js_name = dumpASTNodes)]
pub fn dump_ast_nodes(lang: String, src: String) -> Result<JsValue, JsError> {
  let lang =
    SupportLang::from_str(&lang).map_err(|e| JsError::new(&format!("Language error: {}", e)))?;

  let doc = StrDoc::new(&src, lang);
  let root = AstGrep::doc(doc);
  let debug_node = convert_to_debug_node(root.root());
  let ret = serde_wasm_bindgen::to_value(&debug_node)?;
  Ok(ret)
}

#[wasm_bindgen(js_name = dumpPattern)]
pub fn dump_pattern(
  lang: String,
  query: String,
  selector: Option<String>,
) -> Result<JsValue, JsError> {
  let lang =
    SupportLang::from_str(&lang).map_err(|e| JsError::new(&format!("Language error: {}", e)))?;

  let dumped = dump_pattern_impl(lang, query, selector)?;
  let ret = serde_wasm_bindgen::to_value(&dumped)?;
  Ok(ret)
}
