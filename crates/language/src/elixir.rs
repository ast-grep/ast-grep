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
  test_match(
    r#"
    def $FUNC($$$ARGS) when $GUARDS do
      $$$BODY
    end
    "#,
    r#"
    def add(a, b) when is_integer(a) and is_integer(b) do
      a + b
    end
    "#,
  );
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Elixir)
}

#[test]
fn test_elixir_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    "Stream.map([1, 2, 3], fn x -> x * 2 end)",
    "Stream.map($$$ARGS)",
    "Enum.map($$$ARGS)",
  )?;
  assert_eq!(ret, "Enum.map([1, 2, 3], fn x -> x * 2 end)");

  let ret = test_replace(
    ":budgie = hd([:budgie, :cat, :dog])",
    "$FIRST = hd($LIST)",
    "[$FIRST | _] = $LIST",
  )?;
  assert_eq!(ret, "[:budgie | _] = [:budgie, :cat, :dog]");

  let ret = test_replace(
    "opts[:hostname] || \"localhost\"",
    "opts[$KEY] || $DEFAULT",
    "Keyword.get(opts, $KEY, $DEFAULT)",
  )?;
  assert_eq!(ret, "Keyword.get(opts, :hostname, \"localhost\")");

  let ret = test_replace(
    "Module.function(:a, :b)",
    "Module.function($ARG1, $ARG2)",
    "Module.function($ARG2, $ARG1)",
  )?;
  assert_eq!(ret, "Module.function(:b, :a)");

  let ret = test_replace(
    "Greeter.greet(:hello, \"human\")",
    "Greeter.greet($ARG1, $ARG2)",
    "Greeter.greet($ARG1, name: $ARG2)",
  )?;
  assert_eq!(ret, "Greeter.greet(:hello, name: \"human\")");

  let ret = test_replace(
    "for x <- [\"budgie\", \"cat\", \"dog\"], do: String.to_atom(x)",
    "for $I <- $LIST, do: $MODULE.$FUNCTION($I)",
    "Enum.map($LIST, fn $I -> $MODULE.$FUNCTION($I) end)",
  )?;
  assert_eq!(
    ret,
    "Enum.map([\"budgie\", \"cat\", \"dog\"], fn x -> String.to_atom(x) end)"
  );

  Ok(())
}
