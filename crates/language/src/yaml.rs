#![cfg(test)]
use ast_grep_core::source::TSParseError;

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

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
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
fn test_yaml_replace() -> Result<(), TSParseError> {
  let ret = test_replace(SOURCE, "$KEY: value", "value: $KEY")?;
  assert_eq!(ret, EXPECTED);
  Ok(())
}
