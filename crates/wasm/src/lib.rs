mod utils;

use ast_grep_config::{AstGrepRuleConfig};
use ast_grep_core::language::Language;

use serde::{Serialize, Deserialize};

use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


#[derive(Serialize, Deserialize)]
pub struct MatchResult {
    pub start: usize,
    pub end: usize,
}


#[wasm_bindgen]
pub fn find_nodes(src: &str, config: &JsValue) -> Vec<JsValue> {
    let config: AstGrepRuleConfig = config.into_serde().unwrap();
    let lang = config.language;
    let root = lang.new(src);
    let matcher = config.get_matcher();
    root.root().find_all(matcher).map(|n| {
        JsValue::from_serde(&MatchResult {
            start: n.inner.start_byte(),
            end: n.inner.start_byte(),
        }).unwrap()
    }).collect()
}
