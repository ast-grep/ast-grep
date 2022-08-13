mod utils;

use ast_grep_config::{try_from_serializable, SerializableRule};
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
    let mut parser = tree_sitter::Parser::new().unwrap();
    let lang = web_tree_sitter_sys::Language::load_path("tree-sitter-javascript.wasm")
        .await
        .unwrap();
    let lang = get_lang(lang);
    parser.set_language(&lang).unwrap();
    let config: SerializableRule = config.into_serde().unwrap();
    let root = lang.ast_grep(src);
    let matcher = try_from_serializable(config, lang).unwrap();
    let ret: Vec<_> = root
        .root()
        .find_all(matcher)
        .map(|n| {
            let range = n.range();
            (range.start, range.end)
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
