use crate::parsers::language_cpp;
use ast_grep_core::language::{Language, TSLanguage};
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub struct Cpp;
impl Language for Cpp {
  fn get_ts_language(&self) -> TSLanguage {
    language_cpp()
  }
  // https://en.cppreference.com/w/cpp/language/identifiers
  // Due to some issues in the tree-sitter parser, it is not possible to use
  // unicode literals in identifiers for C/C++ parsers
  fn expando_char(&self) -> char {
    '_'
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    // use stack buffer to reduce allocation
    let mut buf = [0; 4];
    let expando = self.expando_char().encode_utf8(&mut buf);
    // TODO: use more precise replacement
    let replaced = query.replace(self.meta_var_char(), expando);
    Cow::Owned(replaced)
  }
}

#[cfg(test)]
mod test {
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
}
