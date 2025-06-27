#![cfg(test)]

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Pug);
}

#[test]
fn test_pug_pattern() {
  test_match("h1 $TEXT", "h1 Hello World");
  test_match("div(class=$CLASS)", "div(class='container')");
  test_match("$TAG $CONTENT", "p This is a paragraph");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Pug)
}

#[test]
fn test_pug_replace() {
  let ret = test_replace(
    "h1 Hello World",
    "h1 $TEXT",
    "h2 $TEXT",
  );
  assert_eq!(ret, "h2 Hello World");
}