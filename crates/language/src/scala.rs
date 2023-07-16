#![cfg(test)]
//! Standalone Scala file to test syntax.
//! Scala does not need special processing and can be a stub lang.
//! But this file is created for testing Scala2 and Scala3.

use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Scala);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Scala);
}

#[test]
fn test_scala_str() {
  test_match("println($A)", "println(123)");
  test_match("println(\"123\")", "println(\"123\")");
  test_non_match("println(\"123\")", "println(\"456\")");
  test_non_match("\"123\"", "\"456\"");
}

#[test]
fn test_scala_pattern() {
  test_match("val $A = 0", "val a = 0");
  test_match("foo($VAR)", "foo(bar)");
  test_match("type $A = String", "type Foo = String");
  test_match("$A.filter(_ == $B)", "foo.filter(_ == bar)");
  test_match("if ($A) $B else $C", "if (foo) bar else baz");
  // Scala 3 syntax
  test_match("if $A then $B else $C", "if foo then bar else baz");
  test_non_match("if ($A) $B else $C", "if foo then bar else baz");
  test_non_match("type $A = Int", "type Foo = String");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Scala)
}

#[test]
fn test_scala_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    "foo.filter(_ == bar)",
    "$A.filter(_ == $B)",
    "$A.filter(_ == baz)",
  )?;
  assert_eq!(ret, "foo.filter(_ == baz)");
  Ok(())
}
