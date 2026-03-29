#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Dart);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Dart);
}

#[test]
fn test_dart_str() {
  // Test simple string/number matching
  test_match("123", "123");
  test_match("'123'", "'123'");
  test_non_match("'123'", "'456'");
  // Test identifier matching
  test_match("a", "a");
}

#[test]
fn test_dart_pattern() {
  // Test matching just the function name identifier
  test_match("plus", "void plus(int a, int b) { }");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Dart)
}

#[test]
fn test_dart_replace() {
  // Test replacing a simple number literal
  let ret = test_replace(r#"void main() { foo(123); }"#, r#"123"#, r#"456"#);
  assert!(
    ret.contains("456"),
    "expected replacement to contain '456', got: {}",
    ret
  );
}
