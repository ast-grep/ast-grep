#![cfg_attr(target_arch = "wasm32", feature(c_variadic))]

#[cfg(target_arch = "wasm32")]
pub mod wasm_libc;

use ast_grep_core::{AstGrep, Language};
use ast_grep_language::SupportLang;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
#[allow(unused_macros)]
macro_rules! log {
    ($($t:tt)*) => (web_sys::console::log_1(&format!($($t)*).into()))
}

// Initialize panic hook for better error messages
fn init_panic_hook() {
  console_error_panic_hook::set_once();
}

#[derive(Serialize, Deserialize)]
struct Position {
  row: usize,
  column: usize,
}

#[derive(Serialize, Deserialize)]
struct NodeRange {
  start: Position,
  end: Position,
}

// Wrapper for AstGrep to expose to JavaScript
#[wasm_bindgen]
pub struct SgRoot {
  inner: AstGrep<ast_grep_core::StrDoc<ast_grep_language::SupportLang>>,
}

// Wrapper for Node to expose to JavaScript
#[wasm_bindgen]
pub struct SgNode {
  root: SgRoot,
  node: ast_grep_core::Node<'static, ast_grep_core::StrDoc<ast_grep_language::SupportLang>>,
}

#[wasm_bindgen]
impl SgNode {
  #[wasm_bindgen(getter)]
  pub fn text(&self) -> String {
    self.node.text().to_string()
  }

  #[wasm_bindgen(getter)]
  pub fn kind(&self) -> String {
    self.node.kind().to_string()
  }

  #[wasm_bindgen(getter)]
  pub fn range(&self) -> JsValue {
    let byte_range = self.node.range();
    let start_pos = self.node.start_pos();
    let end_pos = self.node.end_pos();

    let result = NodeRange {
      start: Position {
        row: start_pos.line(),
        column: start_pos.column(&self.node),
      },
      end: Position {
        row: end_pos.line(),
        column: end_pos.column(&self.node),
      },
    };

    serde_wasm_bindgen::to_value(&result).unwrap()
  }
}

#[wasm_bindgen]
impl SgRoot {
  #[wasm_bindgen]
  pub fn root(&self) -> Result<SgNode, JsError> {
    // This is a workaround for the lifetime issue
    // We're creating a new SgRoot with the same inner value
    let root = SgRoot {
      inner: AstGrep::new(self.inner.source(), *self.inner.lang()),
    };

    // This is unsafe but necessary due to lifetime constraints
    // The node is tied to the root's lifetime, but we need to return it separately
    let node = unsafe { std::mem::transmute(root.inner.root()) };

    Ok(SgNode { root, node })
  }

  #[wasm_bindgen]
  pub fn source(&self) -> String {
    self.inner.source().to_string()
  }
}

#[wasm_bindgen(js_name = parse)]
pub fn parse(lang: String, src: String) -> Result<SgRoot, JsError> {
  init_panic_hook();

  let lang =
    SupportLang::from_str(&lang).map_err(|e| JsError::new(&format!("Language error: {}", e)))?;

  let ast_grep = AstGrep::new(src, lang);

  Ok(SgRoot { inner: ast_grep })
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

#[wasm_bindgen(start)]
pub fn start() {
  init_panic_hook();
}
