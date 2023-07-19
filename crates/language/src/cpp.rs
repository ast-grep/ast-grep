#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Cpp);
}

#[test]
fn test_cpp_pattern() {
  test_match("$A->b()", "expr->b()");
  test_match("expr->$B()", "expr->b()");
  test_match("ns::ns2::$F()", "ns::ns2::func()");
  test_match("template <typename $T>", "template <typename T>");
  test_match("if constexpr ($C) {}", "if constexpr (13+5==18) {}");
  test_match(
    "template <typename T> typename std::enable_if<$C, T>::type;",
    "template <typename T> typename std::enable_if<std::is_signed<T>::value, T>::type;",
  );
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Cpp)
}

#[test]
fn test_cpp_replace() -> Result<(), TSParseError> {
  let ret = test_replace("expr->b()", "$A->b()", "func($A)->b()")?;
  assert_eq!(ret, "func(expr)->b()");
  Ok(())
}
