#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Haskell);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Haskell);
}

#[test]
fn test_haskell_str() {
  test_match("return $A", "return 3");
  test_match(r#""abc""#, r#""abc""#);
  test_match("$A $B", "f x");
  test_match("$A ($B $C)", "f (x y)");
  test_match("let $A = $B in $A + $A", "let x = 3 in x + x");
  test_non_match("$A $B", "f");
  test_non_match("$A + $A", "3 + 4");
  test_non_match("$A ($B $C)", "f x y");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Html)
}

#[test]
fn test_haskell_replace() -> Result<(), TSParseError> {
  // TODO: Test replacing
  Ok(())
}
