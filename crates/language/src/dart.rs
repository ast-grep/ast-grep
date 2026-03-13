#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Dart);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Dart);
}

#[test]
fn test_dart_class() {
  test_match("class $A {}", "class Foo {}");
  test_non_match("class $A {}", "class Foo { int x = 1; }");
}

#[test]
fn test_dart_class_with_body() {
  test_match("class $A { $$$BODY }", "class Foo { int x = 1; }");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Dart)
}

#[test]
fn test_dart_replace() {
  let ret = test_replace("class Foo {}", "class $A {}", "class $A extends Base {}");
  assert_eq!(ret, "class Foo extends Base {}");
}
