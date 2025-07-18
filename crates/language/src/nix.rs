#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_replace_lang};

fn test_match(s1: &str, s2: &str) {
  test_match_lang(s1, s2, Nix)
}

#[test]
fn test_nix_pattern() {
  test_match("$A + $B", "1 + 2");
  test_match("{ $A = $B; }", "{ foo = bar; }");
  test_match("with $A; $B", "with pkgs; hello");
  test_match("let $A = $B; in $C", "let x = 5; in x + 1");
}

#[test]
fn test_nix_function() {
  test_match("$A: $B", "x: x + 1");
  test_match("{ $A, $B }: $C", "{ foo, bar }: foo + bar");
  test_match("{ $A ? $B }: $C", "{ x ? 5 }: x * 2");
}

#[test]
fn test_nix_list() {
  test_match("[ $A $B ]", "[ 1 2 ]");
  test_match("[ $$$ITEMS ]", "[ 1 2 3 4 5 ]");
}

#[test]
fn test_nix_string() {
  test_match("\"$A\"", "\"hello\"");
  test_match("''$A''", "''multi\nline''");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  test_replace_lang(src, pattern, replacer, Nix)
}

#[test]
fn test_nix_replace() {
  let ret = test_replace("1 + 2", "$A + $B", "$B + $A");
  assert_eq!(ret, "2 + 1");

  let ret = test_replace("{ foo = bar; }", "{ $A = $B; }", "{ $B = $A; }");
  assert_eq!(ret, "{ bar = foo; }");

  let ret = test_replace(
    "let x = 5; in x + 1",
    "let $A = $B; in $C",
    "let $A = $B * 2; in $C",
  );
  assert_eq!(ret, "let x = 5 * 2; in x + 1");
}
