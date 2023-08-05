#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Swift);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Swift);
}

#[test]
fn test_swift_str() {
  test_match("println(\"123\")", "println(\"123\")");
  test_non_match("println(\"123\")", "println(\"456\")");
  test_non_match("\"123\"", "\"456\"");
}

#[test]
fn test_swift_pattern() {
  test_match("fun($A)", "fun(123)");
  test_match("foo($$$)", "foo(1, 2, 3)");
  test_match(
    "foo() { $E in $F }",
    "foo() { s in
      s.a = 123
    }",
  );
  test_non_match("foo($$$) { $E in $F }", "foo(1, 2, 3)");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Swift)
}

const SOURCE: &str = r#"
foo(a: A, b: B, c: C) { s in
  s.a = a
  s.b = b
}"#;
const EXPECTED: &str = r#"
foo(b: B, a: A, c: C) { s in
  s.a = a
  s.b = b
}"#;

#[test]
fn test_swift_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    SOURCE,
    "foo(a: $A, b: $B, c: $C) { $E in $$$F }",
    "foo(b: $B, a: $A, c: $C) { $E in
  $$$F}",
  )?;
  assert_eq!(ret, EXPECTED);
  Ok(())
}
