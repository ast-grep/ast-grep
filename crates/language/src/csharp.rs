#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, CSharp);
}

#[test]
fn test_c_sharp_pattern() {
  let target = "if (table == null) ThrowHelper.ThrowArgumentNullException(nameof(table));";
  test_match("int $A = 0;", "int nint = 0;");
  test_match("ThrowHelper.ThrowArgumentNullException($)", target);
  test_match("ThrowHelper.$", target);
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, CSharp)
}

#[test]
fn test_c_sharp_replace() -> Result<(), TSParseError> {
  let ret = test_replace("int @int = 0;", "int $A = 0", "bool @bool = true")?;
  assert_eq!(ret, "bool @bool = true;");
  Ok(())
}
