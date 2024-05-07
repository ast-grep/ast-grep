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
  // TODO: Basic patterns do not work yet
  // test_match("return $A", "return 3");
  // test_match(r#""$A""#, r#""abc""#);
  test_match("return", "return");
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
