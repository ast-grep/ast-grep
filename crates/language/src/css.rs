use crate::parsers::language_css;
use ast_grep_core::language::{Language, TSLanguage};
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub struct Css;
impl Language for Css {
  fn get_ts_language(&self) -> TSLanguage {
    language_css()
  }
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
    test_match_lang(query, source, Css);
  }

  #[test]
  fn test_c_sharp_pattern() {
    test_match("$A { color: red; }", ".a { color: red; }");
    test_match(".a { color: $COLOR; }", ".a { color: red; }");
    test_match(".a { $PROP: red; }", ".a { color: red; }");
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
    use crate::test::test_replace_lang;
    test_replace_lang(src, pattern, replacer, Css)
  }

  #[test]
  fn test_c_sharp_replace() -> Result<(), TSParseError> {
    let ret = test_replace(
      ".a {color: red; }",
      ".a { color: $COLOR}",
      ".a {background: $COLOR}",
    )?;
    assert_eq!(ret, ".a {background: red}");
    Ok(())
  }
}
