#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Bash);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Bash);
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Bash)
}

#[test]
fn test_bash_pattern() {
  test_match("123", "123");
  test_match("echo $A", "echo test");
  // TODO
  // test_match("echo { $A }", "echo {1..10}");
  test_match("echo $abc", "echo $abc");
}

#[test]
fn test_bash_pattern_no_match() {
  test_non_match("echo $abc", "echo test");
  test_non_match("echo $abc", "echo $ABC");
}

#[test]
fn test_bash_replace() -> Result<(), TSParseError> {
  // TODO: change the replacer to log $A
  let ret = test_replace("echo 123", "echo $A", "log 123")?;
  assert_eq!(ret, "log 123");
  Ok(())
}
