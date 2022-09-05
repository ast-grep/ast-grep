use ast_grep_core::language::{Language, TSLanguage};
use std::borrow::Cow;
use tree_sitter_c_sharp::language as language_c_sharp;

// impl_lang!(CSharp, language_c_sharp);
#[derive(Clone, Copy)]
pub struct CSharp;
impl Language for CSharp {
  fn get_ts_language(&self) -> TSLanguage {
    TSLanguage::from(language_c_sharp())
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
  use super::*;
  use ast_grep_core::{Matcher, Pattern};

  fn test_match(s1: &str, s2: &str) {
    let pattern = Pattern::new(s1, CSharp);
    let cand = CSharp.ast_grep(s2);
    assert!(
      pattern.find_node(cand.root()).is_some(),
      "goal: {}, candidate: {}",
      pattern.root.root().to_sexp(),
      cand.root().to_sexp(),
    );
  }

  #[test]
  fn test_c_sharp_pattern() {
    test_match("int $A = 0;", "int nint = 0;");
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
    let mut source = CSharp.ast_grep(src);
    let replacer = Pattern::new(replacer, CSharp);
    assert!(source.replace(pattern, replacer));
    source.generate()
  }

  #[test]
  fn test_c_shapr_replace() {
    let ret = test_replace("int @int = 0;", "int $A = 0;", "bool @bool = true");
    assert_eq!(ret, "bool @bool = true;");
  }
}
