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
pub async fn find_nodes(src: String, config: JsValue) -> String {
    if tree_sitter::TreeSitter::init().await.is_err() {
        return "".to_string();
    };
    let mut parser = tree_sitter::Parser::new().unwrap_throw();
    let lang = web_tree_sitter_sys::Language::load_path("tree-sitter-javascript.wasm")
        .await
        .unwrap_throw();
    let lang = get_lang(lang);
    parser.set_language(&lang).unwrap_throw();
    let config: SerializableRule = config.into_serde().unwrap_throw();
    let root = lang.ast_grep(src);
    let matcher = deserialize_rule(config, lang).unwrap_throw();
    let ret: Vec<_> = root
        .root()
        .find_all(matcher)
        .map(|n| {
            let start = n.start_pos();
            let end = n.end_pos();
            vec![start.0, start.1, end.0, end.1]
        })
        .collect();
    format!("{:?}", ret)
}

#[cfg(target_arch = "wasm32")]
fn get_lang(lang: web_tree_sitter_sys::Language) -> tree_sitter::Language {
    tree_sitter::Language::from(lang)
}

#[cfg(not(target_arch = "wasm32"))]
fn get_lang(_lang: web_tree_sitter_sys::Language) -> tree_sitter::Language {
    unreachable!()
}
