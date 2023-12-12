#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_replace_lang};
use ast_grep_core::source::TSParseError;

fn test_match(s1: &str, s2: &str) {
  test_match_lang(s1, s2, Rust)
}

#[test]
fn test_rust_pattern() {
  // fix #6
  test_match("Some($A)", "fn test() { Some(123) }");
  test_match(
    "
match $A {
  Some($B) => $B,
  None => $C,
}",
    r#"fn test() {
patterns = match config.include.clone() {
  Some(patterns) => patterns,
  None => Vec::from([cwd
      .join("**/*.toml")
      .normalize()
      .to_string_lossy()
      .into_owned()]),
};
}"#,
  );
}

#[test]
fn test_rust_wildcard_pattern() {
  // fix #412
  test_match("|$A, $B|", "let w = v.into_iter().reduce(|x, y| x + y);");
  test_match("|$$A, $$B|", "let w = v.into_iter().reduce(|x, _| x + x);");
  test_match("let ($$X, $$Y) = $$$T;", "let (_, y) = (1, 2);");
}

#[test]
fn test_rust_spread_syntax() {
  test_match(
    "let ($X, $Y) = $$$T;",
    "let (.., y) = (1,2,3,4,5,6,7,8,9,10);",
  );
  test_match(
    "$C { $$$A, ..$B};",
    r#"User {
    username: String::from(name),
    ..DEFAULT_USER
  };"#,
  );
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  test_replace_lang(src, pattern, replacer, Rust)
}

#[test]
fn test_rust_replace() -> Result<(), TSParseError> {
  let ret = test_replace("fn test() { Some(123) }", "Some($A)", "Ok($A)")?;
  assert_eq!(ret, "fn test() { Ok(123) }");
  let ret = test_replace(
    r#"
patterns = match config.include.clone() {
  Some(patterns) => patterns,
  None => 123,
}"#,
    "match $A {
  Some($B) => $B,
  None => $C,
}",
    "$A.unwrap_or($C)",
  )?;
  assert_eq!(ret, "\npatterns = config.include.clone().unwrap_or(123)");
  Ok(())
}
