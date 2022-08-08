mod utils;

// use ast_grep_config::{AstGrepRuleConfig};
// use ast_grep_core::language::Language;

use serde::{Deserialize, Serialize};

use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Serialize, Deserialize)]
pub struct MatchResult {
    pub start: usize,
    pub end: usize,
}

// pub async fn javascript() -> tree_sitter::Language {
//     let bytes: &[u8] = include_bytes!("../../../node_modules/tree-sitter-javascript/tree-sitter-javascript.wasm");
//     let result = web_tree_sitter_sys::Language::load_bytes(&bytes.into())
//         .await
//         .map(Into::into)
//         .map_err(Into::<tree_sitter::LanguageError>::into)?;
//     Ok(result)
// }

// pub static ID: &str = "javascript";

// pub fn javascript(language: &tree_sitter::Language) -> anyhow::Result<tree_sitter::Parser> {
//     let mut parser = tree_sitter::Parser::new()?;
//     parser.set_language(language)?;
//     Ok(parser)
// }

use web_tree_sitter_sys::ParserError;
type Result<T> = std::result::Result<T, ParserError>;

#[wasm_bindgen]
pub async fn find_nodes(src: String) -> Result<String> {
    tree_sitter::TreeSitter::init().await;
    let mut parser = tree_sitter::Parser::new().unwrap();
    let lang = web_tree_sitter_sys::Language::load_path("tree-sitter-javascript.wasm")
        .await
        .unwrap();
    #[cfg(target_arch = "wasm32")]
    parser
        .set_language(&tree_sitter::Language::from(lang))
        .unwrap();
    Ok(parser
        .parse(&src, None)
        .unwrap()
        .unwrap()
        .root_node()
        .to_sexp()
        .to_string())

    // let config: AstGrepRuleConfig = config.into_serde().unwrap();
    // let lang = config.language;
    // let root = lang.new(src);
    // let matcher = config.get_matcher();
    // root.root().find_all(matcher).map(|n| {
    //     JsValue::from_serde(&MatchResult {
    //         start: n.inner.start_byte(),
    //         end: n.inner.start_byte(),
    //     }).unwrap()
    // }).collect()
}
