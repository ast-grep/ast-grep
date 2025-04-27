#![cfg(test)]

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, CSharp);
}

#[test]
fn test_c_sharp_pattern() {
  let target = "if (table == null) ThrowHelper.ThrowArgumentNullException(nameof(table));";
  test_match("int $A = 0;", "int nint = 0;");
  test_match("ThrowHelper.ThrowArgumentNullException($_)", target);
  test_match("ThrowHelper.$_", target);
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, CSharp)
}

#[test]
fn test_c_sharp_replace() {
  let ret = test_replace("int @int = 0;", "int $A = 0", "bool @bool = true");
  assert_eq!(ret, "bool @bool = true;");
}
