#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Kotlin);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Kotlin);
}

#[test]
fn test_kotlin_str() {
  test_match("println($A)", "println(123)");
  test_match("println('123')", "println('123')");
  test_non_match("println('123')", "println('456')");
  test_non_match("'123'", "'456'");
}

// --- Value-distinction probes for Kotlin literals ---

#[test]
fn test_kotlin_string_literal_value_distinct() {
  // String literals have a named `string_content` child, so content compares.
  test_non_match(r#"val x = "foo""#, r#"val x = "bar""#);
}

#[test]
fn test_kotlin_triple_string_value_distinct() {
  test_non_match("val x = \"\"\"foo\"\"\"", "val x = \"\"\"bar\"\"\"");
}

#[test]
fn test_kotlin_single_char_literal_value_distinct() {
  // Single-char `character_literal` ('a') has only `'` bookend children —
  // content is folded into the parent text. Same shape as the TOML/YAML bug.
  test_non_match("val c = 'a'", "val c = 'b'");
}

#[test]
fn test_kotlin_pattern() {
  test_match("$A = 0", "a = 0");
  test_match(
    r#"fun $A($$$): $B { $$$ }"#,
    r#"
fun plus(a: Int, b: Int): Int {
  return a + b
}"#,
  );
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Kotlin)
}

#[test]
fn test_kotlin_replace() {
  let ret = test_replace(
    r#"
fun plus(a: Int, b: Int): Int {
  return a + b
}"#,
    r#"fun $F($$$): $R { $$$BODY }"#,
    r#"fun $F() { $$$BODY }"#,
  );
  assert_eq!(
    ret,
    r#"
fun plus() { return a + b }"#
  );
}
