#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, R);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, R);
}

#[test]
fn test_r_str() {
  test_match("print($A)", "print(123)");
  test_match("print('123')", "print('123')");
  test_non_match("print('123')", "print('456')");
  test_non_match("'123'", "'456'");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, R)
}

#[test]
fn test_r_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    r#"
if (any(is.na(x)))"#,
    r#"
if (any(is.na($VAR)))"#,
    "if (anyNA($VAR))",
  )?;
  assert_eq!(ret, "\nif (anyNA(x))");

  let ret = test_replace(
    r#"
1 + 1 -> result"#,
    r#"
$COMPUT -> $VAR"#,
    r#"
$VAR <- $COMPUT"#,
  )?;
  assert_eq!(
    ret,
    r#"

result <- 1 + 1"#
  );
  Ok(())
}
