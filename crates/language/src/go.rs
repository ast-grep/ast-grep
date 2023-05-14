use crate::parsers::language_go;
use ast_grep_core::language::{Language, TSLanguage};
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub struct Go;
impl Language for Go {
  fn get_ts_language(&self) -> TSLanguage {
    language_go()
  }
  // we can use any Unicode code point categorized as "Letter"
  // https://go.dev/ref/spec#letter
  fn expando_char(&self) -> char {
    'µ'
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
    test_match_lang(query, source, Go);
  }

  fn test_non_match(query: &str, source: &str) {
    use crate::test::test_non_match_lang;
    test_non_match_lang(query, source, Go);
  }

  #[test]
  fn test_go_str() {
    test_match("print($A)", "print(123)");
    test_match("print('123')", "print('123')");
    test_non_match("print('123')", "print('456')");
    test_non_match("'123'", "'456'");
  }

  #[test]
  fn test_go_pattern() {
    test_match("$A = 0", "a = 0");
    test_match(
      r#"func $A($$$) $B { $$$ }"#,
      r#"
func plus(a int, b int) int {
    return a + b
}"#,
    );
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
    use crate::test::test_replace_lang;
    test_replace_lang(src, pattern, replacer, Go)
  }

  #[test]
  fn test_go_replace() -> Result<(), TSParseError> {
    let ret = test_replace(
      r#"
func intSeq() {
    defer func()  {
        i++
    }()
}"#,
      r#"defer func() {
  $$$BODY }()"#,
      r#"func b() { $$$BODY}"#,
    )?;
    assert_eq!(
      ret,
      r#"
func intSeq() {
    func b() { i++
}
}"#
    );
    Ok(())
  }
}
