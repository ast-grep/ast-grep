#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Yaml);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Yaml);
}

#[test]
fn test_yaml_str() {
  test_match("123", "123");
  test_non_match("123", "'123'");
  // the pattern below should not match but match now
  // test_non_match("\"123\"", "\"456\"");
}

#[test]
fn test_yaml_pattern() {
  test_match("foo: $BAR", "foo: 123");
  test_match("foo: $$$", "foo: [1, 2, 3]");
  test_match(
    "foo: $BAR",
    "foo:
      - a
    ",
  );
  test_non_match("foo: $BAR", "bar: bar");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Yaml)
}

const SOURCE: &str = r#"
key: value
list:
  - item1
  - item2
"#;
const EXPECTED: &str = r#"
value: key
list:
  - item1
  - item2
"#;

#[test]
fn test_yaml_replace() {
  let ret = test_replace(SOURCE, "$KEY: value", "value: $KEY");
  assert_eq!(ret, EXPECTED);
}

#[test]
fn test_yaml_int_value_distinct() {
  test_non_match("a: 100", "a: 200");
}

#[test]
fn test_yaml_bool_value_distinct() {
  test_non_match("a: true", "a: false");
}

#[test]
fn test_yaml_plain_string_value_distinct() {
  // Plain (unquoted) scalars: the value text IS the node's text.
  test_non_match("a: foo", "a: bar");
}

#[test]
fn test_yaml_quoted_string_value_distinct() {
  test_non_match(r#"a: "foo""#, r#"a: "bar""#);
}

#[test]
fn test_yaml_single_quoted_string_value_distinct() {
  test_non_match("a: 'foo'", "a: 'bar'");
}

#[test]
fn test_yaml_block_literal_value_distinct() {
  // Literal block scalar (`|`)
  test_non_match("a: |\n  foo\n", "a: |\n  bar\n");
}

#[test]
fn test_yaml_block_folded_value_distinct() {
  // Folded block scalar (`>`)
  test_non_match("a: >\n  foo\n", "a: >\n  bar\n");
}

#[test]
fn test_yaml_string_replace_respects_value() {
  let mut source = Yaml.ast_grep("a: foo\n");
  let replaced = source
    .replace("a: bar", "a: baz")
    .expect("should parse");
  assert!(!replaced, "should not match a different string value");
  assert_eq!(source.generate(), "a: foo\n");
}
