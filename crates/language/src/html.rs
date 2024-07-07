use super::pre_process_pattern;
use ast_grep_core::language::{InjectionExtractor, TSNode, TSRange};

// tree-sitter-html uses locale dependent iswalnum for tagName
// https://github.com/tree-sitter/tree-sitter-html/blob/b5d9758e22b4d3d25704b72526670759a9e4d195/src/scanner.c#L194
#[derive(Clone, Copy)]
pub struct Html;
impl ast_grep_core::language::Language for Html {
  fn get_ts_language(&self) -> ast_grep_core::language::TSLanguage {
    crate::parsers::language_html()
  }
  fn expando_char(&self) -> char {
    'z'
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> std::borrow::Cow<'q, str> {
    pre_process_pattern(self.expando_char(), query)
  }
  fn extract_injections(&self) -> Option<InjectionExtractor> {
    Some(InjectionExtractor {
      injectable_languages: &["css", "javascript"],
      extract_injections,
    })
  }
}

fn extract_injections(root: TSNode) -> Vec<(String, Vec<TSRange>)> {
  // TODO!
  vec![]
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::source::TSParseError;

  fn test_match(query: &str, source: &str) {
    use crate::test::test_match_lang;
    test_match_lang(query, source, Html);
  }

  fn test_non_match(query: &str, source: &str) {
    use crate::test::test_non_match_lang;
    test_non_match_lang(query, source, Html);
  }

  #[test]
  fn test_html_match() {
    test_match("<input>", "<input>");
    test_match("<$TAG>", "<input>");
    test_match("<$TAG class='foo'>$$$</$TAG>", "<div class='foo'></div>");
    test_match("<div>$$$</div>", "<div>123</div>");
    test_non_match("<$TAG class='foo'>$$$</$TAG>", "<div></div>");
    test_non_match("<div>$$$</div>", "<div class='foo'>123</div>");
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
    use crate::test::test_replace_lang;
    test_replace_lang(src, pattern, replacer, Html)
  }

  #[test]
  fn test_html_replace() -> Result<(), TSParseError> {
    let ret = test_replace(
      r#"<div class='foo'>bar</div>"#,
      r#"<$TAG class='foo'>$$$B</$TAG>"#,
      r#"<$TAG class='$$$B'>foo</$TAG>"#,
    )?;
    assert_eq!(ret, r#"<div class='bar'>foo</div>"#);
    Ok(())
  }
}
