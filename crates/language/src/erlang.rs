#![cfg(test)]

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Erlang);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Erlang);
}

#[test]
fn test_erlang_str() {
  test_match("io:format($A)", "io:format(\"Hello\")");
  test_non_match("io:format(\"Hello\")", "io:format(\"World\")");
  test_non_match("\"Hello\"", "\"World\"");
}

#[test]
fn test_erlang_pattern() {
  test_match("$A", "ok");
  test_match("$A =:= $B", "X =:= Y");
  test_match("$F($$$ARGS)", "foo(1, 2, 3)");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Erlang)
}

#[test]
fn test_erlang_module_attribute() {
  test_match("-module($NAME).", "-module(foo).");
  test_non_match("-module($NAME).", "-behaviour(gen_server).");
}

#[test]
fn test_erlang_replace() {
  let ret = test_replace(
    "lists:map(Fun, List)",
    "lists:map($FUN, $LIST)",
    "lists:filtermap($FUN, $LIST)",
  );
  assert_eq!(ret, "lists:filtermap(Fun, List)");
}
