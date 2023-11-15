#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;
use ast_grep_core::Pattern;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Ruby);
}

#[test]
fn test_ruby_pattern() {
  test_match("Foo::bar", "Foo::bar");
}

// https://github.com/ast-grep/ast-grep/issues/713
#[test]
fn test_ruby_tree_sitter_panic() {
  let pattern = Pattern::str("Foo::barbaz", Ruby);
  assert_eq!(pattern.fixed_string(), "barbaz");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Ruby)
}

#[test]
fn test_ruby_replace() -> Result<(), TSParseError> {
  let ret = test_replace("Foo::bar()", "Foo::$METHOD()", "$METHOD()")?;
  assert_eq!(ret, "bar()");
  Ok(())
}
