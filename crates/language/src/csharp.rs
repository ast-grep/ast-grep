use crate::parsers::language_c_sharp;
use ast_grep_core::language::{Language, TSLanguage};
use std::borrow::Cow;

// impl_lang!(CSharp, language_c_sharp);
#[derive(Clone, Copy)]
pub struct CSharp;
impl Language for CSharp {
  fn get_ts_language(&self) -> TSLanguage {
    language_c_sharp()
  }
  // https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#643-identifiers
  // all letter number is accepted
  // https://www.compart.com/en/unicode/category/Nl
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
}
