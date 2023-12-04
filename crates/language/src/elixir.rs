#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Elixir);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Elixir);
}

#[test]
fn test_elixir_str() {
  test_match("IO.puts(\"$A\")", "IO.puts(\"123\")");
  test_match("IO.puts($A)", "IO.puts(123)");
  test_non_match("IO.puts(123)", "IO.puts(456)");
  test_non_match("\"123\"", "\"456\"");
}

#[test]
fn test_elixir_pattern() {
  test_match("$A", ":ok");
  test_match("$A != nil", "a != nil");
  test_match(r#"IO.inspect($A)"#, r#"IO.inspect(:hello)"#);
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Elixir)
}

#[test]
fn test_elixir_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    r#"Stream.map([1, 2, 3], fn x -> x * 2 end)"#,
    r#"Stream.map($$$ARGS)"#,
    r#"Enum.map($$$ARGS)"#,
  )?;
  assert_eq!(ret, r#"Enum.map([1, 2, 3], fn x -> x * 2 end)"#);
  Ok(())
}
