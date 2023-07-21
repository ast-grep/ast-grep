#![cfg(test)]
use ast_grep_core::source::TSParseError;

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

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Json)
}

#[test]
fn test_json_replace() -> Result<(), TSParseError> {
  let ret = test_replace(r#"{ "a": 123 }"#, r#"123"#, r#"456"#)?;
  assert_eq!(ret, r#"{ "a": 456 }"#);
  Ok(())
}
