#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Json);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Json);
}

#[test]
fn test_json_str() {
  test_match("123", "123");
  test_match("{\"d\": 123}", "{\"d\": 123}");
  test_non_match("344", "123");
  test_non_match("{\"key\": 123}", "{}");
}

#[test]
fn test_json_pattern() {
  test_match("$A", "123");
  test_match(r#"[$A]"#, r#"[123]"#);
  test_match(r#"{ $$$ }"#, r#"{"abc": 123}"#);
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Json)
}

#[test]
fn test_json_replace() {
  let ret = test_replace(r#"{ "a": 123 }"#, r#"123"#, r#"456"#);
  assert_eq!(ret, r#"{ "a": 456 }"#);
}

// --- Value-distinction tests: same shape as the TOML suite, to confirm
//     JSON does NOT suffer the same string-value-comparison bug.

#[test]
fn test_json_number_value_distinct() {
  test_non_match(r#"{"a": 100}"#, r#"{"a": 200}"#);
}

#[test]
fn test_json_bool_value_distinct() {
  test_non_match(r#"{"a": true}"#, r#"{"a": false}"#);
}

#[test]
fn test_json_string_value_distinct() {
  // tree-sitter-json exposes string contents as named `string_content`, so
  // unlike TOML this test passes without any matcher patch.
  test_non_match(r#"{"a": "foo"}"#, r#"{"a": "bar"}"#);
}

#[test]
fn test_json_string_replace_respects_value() {
  let mut source = Json.ast_grep(r#"{"a": "foo"}"#);
  let replaced = source
    .replace(r#""bar""#, r#""baz""#)
    .expect("should parse");
  assert!(!replaced, "should not match a different string value");
  assert_eq!(source.generate(), r#"{"a": "foo"}"#);
}
