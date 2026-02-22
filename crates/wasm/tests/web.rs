//! Integration tests for ast-grep-wasm.
//!
//! Run with:
//! ```bash
//! cd crates/wasm
//! npm install
//! wasm-pack test --node
//! ```
#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use wasm::WasmLangInfo;

#[wasm_bindgen(module = "/tests/setup.js")]
extern "C" {
  #[wasm_bindgen(js_name = "parserPath")]
  fn parser_path(lang: &str) -> String;
}

fn custom_lang(name: &str) -> WasmLangInfo {
  let expando_char = match name {
    "python" | "c" | "cpp" | "csharp" | "elixir" | "go" | "haskell" | "kotlin" | "php" | "ruby"
    | "rust" | "swift" => Some('µ'),
    "css" | "nix" => Some('_'),
    "html" => Some('z'),
    _ => None,
  };
  WasmLangInfo {
    library_path: parser_path(name),
    expando_char,
  }
}

async fn register_langs(names: &[&str]) {
  let langs: HashMap<String, WasmLangInfo> = names
    .iter()
    .map(|name| (name.to_string(), custom_lang(name)))
    .collect();
  wasm::register_dynamic_language(serde_wasm_bindgen::to_value(&langs).unwrap())
    .await
    .unwrap();
}

async fn setup() {
  wasm::initialize_tree_sitter().await.unwrap();
  register_langs(&["javascript"]).await;
}

fn js_parse(src: &str) -> wasm::SgRoot {
  wasm::parse("javascript".into(), src.into()).unwrap()
}

fn js_kind(name: &str) -> JsValue {
  let k = wasm::kind("javascript".into(), name.into()).unwrap();
  JsValue::from_f64(k as f64)
}

fn make_config(json_str: &str) -> JsValue {
  use serde::Serialize;
  let val: serde_json::Value = serde_json::from_str(json_str).unwrap();
  val
    .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
    .unwrap()
}

// --- Basic parsing ---

#[wasm_bindgen_test]
async fn test_parse_root() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let root = sg.root();
  assert_eq!(root.kind(), "program");
  assert!(!root.text().is_empty());
  assert_eq!(sg.filename(), "anonymous");
}

// --- Find by pattern ---

#[wasm_bindgen_test]
async fn test_find_by_pattern() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let matched = sg
    .root()
    .find(JsValue::from_str("console.log"))
    .unwrap()
    .unwrap();
  let range = matched.range();
  assert_eq!(range.start.line, 0);
  assert_eq!(range.start.column, 0);
  assert_eq!(range.start.index, 0);
  assert_eq!(range.end.line, 0);
  assert_eq!(range.end.column, 11);
  assert_eq!(range.end.index, 11);
}

#[wasm_bindgen_test]
async fn test_find_nested() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let outer = sg
    .root()
    .find(JsValue::from_str("console.log"))
    .unwrap()
    .unwrap();
  let inner = outer.find(JsValue::from_str("console")).unwrap().unwrap();
  let range = inner.range();
  assert_eq!(range.start.line, 0);
  assert_eq!(range.start.column, 0);
  assert_eq!(range.start.index, 0);
  assert_eq!(range.end.line, 0);
  assert_eq!(range.end.column, 7);
  assert_eq!(range.end.index, 7);
}

#[wasm_bindgen_test]
async fn test_find_not_match() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let result = sg.root().find(JsValue::from_str("notExist")).unwrap();
  assert!(result.is_none());
}

// --- Find all ---

#[wasm_bindgen_test]
async fn test_find_all() {
  setup().await;
  let sg = js_parse("console.log(123); let a = console.log.bind(console);");
  let matches = sg
    .root()
    .find_all(JsValue::from_str("console.log"))
    .unwrap();
  assert_eq!(matches.len(), 2);
  let r0 = matches[0].range();
  assert_eq!(r0.start.index, 0);
  assert_eq!(r0.end.index, 11);
  let r1 = matches[1].range();
  assert_eq!(r1.start.index, 26);
  assert_eq!(r1.end.index, 37);
}

// --- Find by kind ---

#[wasm_bindgen_test]
async fn test_find_by_kind() {
  setup().await;
  let sg = js_parse("console.log(\"hello world\")");
  let matched = sg
    .root()
    .find(js_kind("member_expression"))
    .unwrap()
    .unwrap();
  let range = matched.range();
  assert_eq!(range.start.index, 0);
  assert_eq!(range.end.index, 11);
}

// --- Find by config ---

#[wasm_bindgen_test]
async fn test_find_by_config() {
  setup().await;
  let sg = js_parse("console.log(\"hello world\")");
  let config = make_config(r#"{"rule": {"kind": "member_expression"}}"#);
  let matched = sg.root().find(config).unwrap().unwrap();
  let range = matched.range();
  assert_eq!(range.start.index, 0);
  assert_eq!(range.end.index, 11);
}

// --- Meta variables ---

#[wasm_bindgen_test]
async fn test_get_variable() {
  setup().await;
  let sg = js_parse("console.log(\"hello world\")");
  let matched = sg
    .root()
    .find(JsValue::from_str("console.log($MATCH)"))
    .unwrap()
    .unwrap();
  let var_node = matched.get_match("MATCH".into()).unwrap();
  assert_eq!(var_node.text(), "\"hello world\"");
}

#[wasm_bindgen_test]
async fn test_find_multiple_nodes() {
  setup().await;
  let sg = js_parse("a(1, 2, 3)");
  let matched = sg
    .root()
    .find(JsValue::from_str("a($$$B)"))
    .unwrap()
    .unwrap();
  let range = matched.range();
  assert_eq!(range.start.index, 0);
  assert_eq!(range.end.index, 10);
  let vars = matched.get_multiple_matches("B".into());
  let first = vars.first().unwrap().range();
  let last = vars.last().unwrap().range();
  assert_eq!(first.start.index, 2);
  assert_eq!(last.end.index, 9);
}

// --- Unicode ---

#[wasm_bindgen_test]
async fn test_find_unicode() {
  setup().await;
  let src = "console.log(\"Hello, 世界\")\n  print(\"ザ・ワールド\")";
  let sg = js_parse(src);
  let m1 = sg
    .root()
    .find(JsValue::from_str("console.log($_)"))
    .unwrap()
    .unwrap();
  let r1 = m1.range();
  assert_eq!(r1.start.line, 0);
  assert_eq!(r1.start.column, 0);
  assert_eq!(r1.end.line, 0);

  let m2 = sg
    .root()
    .find(JsValue::from_str("print($_)"))
    .unwrap()
    .unwrap();
  let r2 = m2.range();
  assert_eq!(r2.start.line, 1);
  assert_eq!(r2.start.column, 2);
}

// --- Transformation ---

#[wasm_bindgen_test]
async fn test_find_with_transformation() {
  setup().await;
  let sg = js_parse("console.log(\"Hello, 世界\")");
  let config = make_config(
    r#"{
      "rule": {"pattern": "console.log($A)"},
      "transform": {
        "NEW_ARG": {
          "substring": {"source": "$A", "startChar": 1, "endChar": -1}
        }
      }
    }"#,
  );
  let matched = sg.root().find(config).unwrap().unwrap();
  assert_eq!(
    matched.get_transformed("NEW_ARG".into()).unwrap(),
    "Hello, 世界"
  );
  assert_eq!(
    matched.get_match("A".into()).unwrap().text(),
    "\"Hello, 世界\""
  );
}

// --- Code fix ---

#[wasm_bindgen_test]
async fn test_code_fix() {
  setup().await;
  let sg = js_parse("a = console.log(123)");
  let matched = sg
    .root()
    .find(JsValue::from_str("console.log"))
    .unwrap()
    .unwrap();
  let fix = matched.replace("console.error".into());
  assert_eq!(fix.inserted_text, "console.error");
  assert_eq!(fix.start_pos, 4);
  assert_eq!(fix.end_pos, 15);

  let edits_val = serde_wasm_bindgen::to_value(&vec![&fix]).unwrap();
  let new_code = sg.root().commit_edits(edits_val).unwrap();
  assert_eq!(new_code, "a = console.error(123)");
}

#[wasm_bindgen_test]
async fn test_multiple_fixes() {
  setup().await;
  let sg = js_parse("いいよ = log(123) + log(456)");
  let matches = sg.root().find_all(js_kind("number")).unwrap();
  let mut fixes: Vec<_> = matches.iter().map(|m| m.replace("114514".into())).collect();
  fixes.sort_by(|a, b| b.start_pos.cmp(&a.start_pos));
  let edits_val = serde_wasm_bindgen::to_value(&fixes).unwrap();
  let new_code = sg.root().commit_edits(edits_val).unwrap();
  assert_eq!(new_code, "いいよ = log(114514) + log(114514)");
}

#[wasm_bindgen_test]
async fn test_fix_with_user_range() {
  setup().await;
  let sg = js_parse("いいよ = log(123)");
  let matched = sg.root().find(js_kind("number")).unwrap().unwrap();
  let mut edit = matched.replace("514".into());
  edit.start_pos -= 1;
  edit.end_pos += 1;
  let edits_val = serde_wasm_bindgen::to_value(&vec![&edit]).unwrap();
  let new_code = sg.root().commit_edits(edits_val).unwrap();
  assert_eq!(new_code, "いいよ = log514");
}

// --- Matcher methods ---

#[wasm_bindgen_test]
async fn test_node_matches_pattern() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let matched = sg
    .root()
    .find(make_config(r#"{"rule": {"kind": "call_expression"}}"#))
    .unwrap()
    .unwrap();
  assert!(matched
    .matches(JsValue::from_str("console.log($$$)"))
    .unwrap());
  assert!(!matched.matches(JsValue::from_str("console.log")).unwrap());
}

#[wasm_bindgen_test]
async fn test_node_matches_config() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let matched = sg
    .root()
    .find(JsValue::from_str("console.log($$$)"))
    .unwrap()
    .unwrap();
  assert!(matched
    .matches(make_config(r#"{"rule": {"kind": "call_expression"}}"#))
    .unwrap());
  assert!(!matched
    .matches(make_config(r#"{"rule": {"kind": "identifier"}}"#))
    .unwrap());
}

#[wasm_bindgen_test]
async fn test_node_follows() {
  setup().await;
  let sg = js_parse("const a = 1; const b = 2;");
  let a = sg
    .root()
    .find(JsValue::from_str("const a = 1"))
    .unwrap()
    .unwrap();
  let b = sg
    .root()
    .find(JsValue::from_str("const b = 2"))
    .unwrap()
    .unwrap();
  assert!(!a.follows(JsValue::from_str("const b = 2")).unwrap());
  assert!(b.follows(JsValue::from_str("const a = 1")).unwrap());
}

#[wasm_bindgen_test]
async fn test_node_precedes() {
  setup().await;
  let sg = js_parse("const a = 1; const b = 2;");
  let a = sg
    .root()
    .find(JsValue::from_str("const a = 1"))
    .unwrap()
    .unwrap();
  let b = sg
    .root()
    .find(JsValue::from_str("const b = 2"))
    .unwrap()
    .unwrap();
  assert!(a.precedes(JsValue::from_str("const b = 2")).unwrap());
  assert!(!b.precedes(JsValue::from_str("const a = 1")).unwrap());
}

#[wasm_bindgen_test]
async fn test_node_inside() {
  setup().await;
  let sg = js_parse("if (true) { const x = 1; }");
  let matched = sg
    .root()
    .find(JsValue::from_str("const x = 1"))
    .unwrap()
    .unwrap();
  assert!(matched
    .inside(JsValue::from_str("if (true) { $$$ }"))
    .unwrap());
  assert!(!matched
    .inside(JsValue::from_str("function() { $$$ }"))
    .unwrap());
}

#[wasm_bindgen_test]
async fn test_node_has() {
  setup().await;
  let sg = js_parse("if (true) { const x = 1; }");
  let matched = sg
    .root()
    .find(JsValue::from_str("if (true) { $$$ }"))
    .unwrap()
    .unwrap();
  assert!(matched.has(JsValue::from_str("const x = 1")).unwrap());
  assert!(!matched.has(JsValue::from_str("const y = 2")).unwrap());
}

// --- Node properties ---

#[wasm_bindgen_test]
async fn test_node_id() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let by_pattern = sg
    .root()
    .find(JsValue::from_str("console.log($$$)"))
    .unwrap()
    .unwrap();
  let by_kind = sg.root().find(js_kind("call_expression")).unwrap().unwrap();
  assert_eq!(by_pattern.id(), by_kind.id());
}

#[wasm_bindgen_test]
async fn test_node_properties() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let root = sg.root();
  // program node is not a leaf
  assert!(!root.is_leaf());
  assert!(root.is_named());

  // find a number literal — it should be a named leaf
  let num = root.find(js_kind("number")).unwrap().unwrap();
  assert!(num.is_named_leaf());
  assert!(num.is_named());
  assert_eq!(num.text(), "123");
}

#[wasm_bindgen_test]
async fn test_node_is_kind() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let num = sg.root().find(js_kind("number")).unwrap().unwrap();
  assert!(num.is("number".into()));
  assert!(!num.is("string".into()));
}

// --- Tree traversal ---

#[wasm_bindgen_test]
async fn test_children_and_parent() {
  setup().await;
  let sg = js_parse("a; b; c;");
  let root = sg.root();
  let children = root.children_nodes();
  // program has 3 expression statements (+ possible semicolons)
  let named: Vec<_> = children.iter().filter(|c| c.is_named()).collect();
  assert!(named.len() >= 3);

  // parent of first child should be program
  let parent = named[0].parent_node().unwrap();
  assert_eq!(parent.kind(), "program");
}

#[wasm_bindgen_test]
async fn test_next_prev() {
  setup().await;
  let sg = js_parse("const a = 1; const b = 2;");
  let a = sg
    .root()
    .find(JsValue::from_str("const a = 1"))
    .unwrap()
    .unwrap();
  let next = a.next_node();
  assert!(next.is_some());
  let b = sg
    .root()
    .find(JsValue::from_str("const b = 2"))
    .unwrap()
    .unwrap();
  let prev = b.prev_node();
  assert!(prev.is_some());
}

#[wasm_bindgen_test]
async fn test_next_all_prev_all() {
  setup().await;
  let sg = js_parse("a; b; c;");
  let first = sg.root().child_node(0).unwrap();
  let after = first.next_all();
  assert!(after.len() >= 2);
  let last_named: Vec<_> = after.iter().filter(|n| n.is_named()).collect();
  if let Some(last) = last_named.last() {
    let before = last.prev_all();
    assert!(before.len() >= 2);
  }
}

#[wasm_bindgen_test]
async fn test_ancestors() {
  setup().await;
  let sg = js_parse("if (true) { const x = 1; }");
  let x = sg
    .root()
    .find(JsValue::from_str("const x = 1"))
    .unwrap()
    .unwrap();
  let ancestors = x.ancestors();
  // should have at least: statement_block, if_statement, program
  assert!(ancestors.len() >= 2);
  let kinds: Vec<_> = ancestors.iter().map(|a| a.kind()).collect();
  assert!(kinds.contains(&"program".to_string()));
}

#[wasm_bindgen_test]
async fn test_child_by_index() {
  setup().await;
  let sg = js_parse("a; b;");
  let root = sg.root();
  let first = root.child_node(0);
  assert!(first.is_some());
  assert!(first.unwrap().is_named());
}

// --- Field access ---

#[wasm_bindgen_test]
async fn test_field_node() {
  setup().await;
  let sg = js_parse("function foo(a, b) { return a; }");
  let func = sg
    .root()
    .find(js_kind("function_declaration"))
    .unwrap()
    .unwrap();
  let name = func.field_node("name".into());
  assert!(name.is_some());
  assert_eq!(name.unwrap().text(), "foo");
}

// --- Top-level functions ---

#[wasm_bindgen_test]
async fn test_kind_function() {
  setup().await;
  let k = wasm::kind("javascript".into(), "identifier".into()).unwrap();
  assert!(k > 0);
}

#[wasm_bindgen_test]
async fn test_pattern_function() {
  setup().await;
  let result = wasm::pattern("javascript".into(), "console.log($A)".into());
  assert!(result.is_ok());
}

// --- dumpPattern ---

fn get_str(obj: &JsValue, key: &str) -> String {
  js_sys::Reflect::get(obj, &key.into())
    .unwrap()
    .as_string()
    .unwrap_or_default()
}

fn get_u32(obj: &JsValue, key: &str) -> u32 {
  js_sys::Reflect::get(obj, &key.into())
    .unwrap()
    .as_f64()
    .unwrap_or(0.0) as u32
}

fn get_pos(obj: &JsValue, key: &str) -> JsValue {
  js_sys::Reflect::get(obj, &key.into()).unwrap()
}

#[wasm_bindgen_test]
async fn test_dump_pattern_simple() {
  setup().await;
  // '$VAR' is 4 chars; JS expando is '$' so no preprocessing changes the string
  let dump = wasm::dump_pattern("javascript".into(), "$VAR".into(), None, None).unwrap();
  assert_eq!(get_str(&dump, "pattern"), "metaVar");
  assert_eq!(get_str(&dump, "text"), "$VAR");
  let start = get_pos(&dump, "start");
  assert_eq!(get_u32(&start, "line"), 0);
  assert_eq!(get_u32(&start, "column"), 0);
  let end = get_pos(&dump, "end");
  assert_eq!(get_u32(&end, "line"), 0);
  assert_eq!(get_u32(&end, "column"), 4);
}

#[wasm_bindgen_test]
async fn test_dump_pattern_nested() {
  setup().await;
  // 'console.log($MSG)' = 17 chars; '(' at col 11, '$MSG' spans col 12–16
  let dump =
    wasm::dump_pattern("javascript".into(), "console.log($MSG)".into(), None, None).unwrap();
  assert_eq!(get_str(&dump, "kind"), "call_expression");
  assert_eq!(get_str(&dump, "pattern"), "internal");
  let start = get_pos(&dump, "start");
  assert_eq!(get_u32(&start, "line"), 0);
  assert_eq!(get_u32(&start, "column"), 0);
  let end = get_pos(&dump, "end");
  assert_eq!(get_u32(&end, "column"), 17);
  // find $MSG metavar inside arguments
  let children = js_sys::Array::from(&js_sys::Reflect::get(&dump, &"children".into()).unwrap());
  assert!(children.length() >= 2);
  let args = (0..children.length())
    .map(|i| children.get(i))
    .find(|c| get_str(c, "kind") == "arguments")
    .expect("arguments node");
  let arg_children = js_sys::Array::from(&get_pos(&args, "children"));
  let meta_var = (0..arg_children.length())
    .map(|i| arg_children.get(i))
    .find(|c| get_str(c, "pattern") == "metaVar")
    .expect("metaVar node");
  assert_eq!(get_str(&meta_var, "text"), "$MSG");
  assert_eq!(get_u32(&get_pos(&meta_var, "start"), "column"), 12);
  assert_eq!(get_u32(&get_pos(&meta_var, "end"), "column"), 16);
}

#[wasm_bindgen_test]
async fn test_dump_pattern_with_selector() {
  setup().await;
  // 'class A { $F = $I }': field_definition at col 10–17
  // $F at col 10–12, $I at col 15–17
  let dump = wasm::dump_pattern(
    "javascript".into(),
    "class A { $F = $I }".into(),
    Some("field_definition".into()),
    None,
  )
  .unwrap();
  assert_eq!(get_str(&dump, "kind"), "field_definition");
  assert_eq!(get_str(&dump, "pattern"), "internal");
  let start = get_pos(&dump, "start");
  assert_eq!(get_u32(&start, "line"), 0);
  assert_eq!(get_u32(&start, "column"), 10);
  let end = get_pos(&dump, "end");
  assert_eq!(get_u32(&end, "column"), 17);
}

#[wasm_bindgen_test]
async fn test_dump_pattern_with_strictness() {
  setup().await;
  // 'let $A = $B' = 11 chars; strictness only affects matching, not position dump
  let dump = wasm::dump_pattern(
    "javascript".into(),
    "let $A = $B".into(),
    None,
    Some("ast".into()),
  )
  .unwrap();
  assert_eq!(get_str(&dump, "kind"), "lexical_declaration");
  let start = get_pos(&dump, "start");
  assert_eq!(get_u32(&start, "line"), 0);
  assert_eq!(get_u32(&start, "column"), 0);
  let end = get_pos(&dump, "end");
  assert_eq!(get_u32(&end, "column"), 11);
}

#[wasm_bindgen_test]
async fn test_dump_pattern_invalid() {
  setup().await;
  let result = wasm::dump_pattern("javascript".into(), "".into(), None, None);
  assert!(result.is_err());
}

// --- Error handling ---

#[wasm_bindgen_test]
async fn test_invalid_language() {
  setup().await;
  let result = wasm::parse("not_a_language".into(), "code".into());
  assert!(result.is_err());
}

#[wasm_bindgen_test]
async fn test_invalid_config() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let config = make_config(r#"{"rule": {"regex": "("}}"#);
  let result = sg.root().find(config);
  assert!(result.is_err());
}

// --- Multi-language support ---

async fn setup_multi_lang() {
  wasm::initialize_tree_sitter().await.unwrap();
  register_langs(&["javascript", "python"]).await;
}

#[wasm_bindgen_test]
async fn test_parse_multiple_languages() {
  setup_multi_lang().await;

  // Parse JavaScript
  let js_sg = wasm::parse("javascript".into(), "console.log(123)".into()).unwrap();
  let js_root = js_sg.root();
  assert_eq!(js_root.kind(), "program");
  let js_match = js_root.find(JsValue::from_str("console.log")).unwrap();
  assert!(js_match.is_some());

  // Parse Python
  let py_sg = wasm::parse("python".into(), "print('hello')".into()).unwrap();
  let py_root = py_sg.root();
  assert_eq!(py_root.kind(), "module");
  let py_match = py_root.find(JsValue::from_str("print('hello')")).unwrap();
  assert!(py_match.is_some());

  // JavaScript still works after loading Python
  let js_sg2 = wasm::parse("javascript".into(), "let x = 1".into()).unwrap();
  let js_match2 = js_sg2.root().find(JsValue::from_str("let x = 1")).unwrap();
  assert!(js_match2.is_some());
}

#[wasm_bindgen_test]
async fn test_kind_multiple_languages() {
  setup_multi_lang().await;

  let js_kind_id = wasm::kind("javascript".into(), "identifier".into()).unwrap();
  let py_kind_id = wasm::kind("python".into(), "identifier".into()).unwrap();
  assert!(js_kind_id > 0);
  assert!(py_kind_id > 0);
}

// --- get_inner_tree ---

#[wasm_bindgen_test]
async fn test_get_inner_tree_root_node() {
  setup().await;
  let sg = js_parse("console.log(123)");
  let tree = sg.get_inner_tree();
  let root = tree.root_node();
  assert_eq!(String::from(root.type_()), "program");
}

#[wasm_bindgen_test]
async fn test_get_inner_tree_has_children() {
  setup().await;
  let sg = js_parse("a; b; c;");
  let tree = sg.get_inner_tree();
  let root = tree.root_node();
  assert!(root.child_count() >= 3);
}

#[wasm_bindgen_test]
async fn test_get_inner_tree_walk() {
  setup().await;
  let sg = js_parse("let x = 1");
  let tree = sg.get_inner_tree();
  let cursor = tree.walk();
  assert_eq!(String::from(cursor.node_type()), "program");
  assert!(cursor.goto_first_child());
  assert_eq!(String::from(cursor.node_type()), "lexical_declaration");
}
