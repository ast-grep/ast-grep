use super::pre_process_pattern;
use ast_grep_core::language::{TSPoint, TSRange};
use ast_grep_core::{matcher::KindMatcher, Doc, Node};
use std::collections::HashMap;

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
  fn injectable_languages(&self) -> Option<&'static [&'static str]> {
    Some(&["css", "js", "ts", "tsx", "scss", "less", "stylus", "coffee"])
  }
  fn extract_injections<D: Doc>(
    &self,
    root: Node<D>,
    conv: impl Fn(Self) -> D::Lang,
  ) -> HashMap<String, Vec<TSRange>> {
    let lang = conv(*self);
    let mut map = HashMap::new();
    let matcher = KindMatcher::new("script_element", lang.clone());
    for script in root.find_all(matcher) {
      let injected = find_lang(&script).unwrap_or_else(|| "js".into());
      let content = script.children().find(|c| c.kind() == "raw_text");
      if let Some(content) = content {
        map
          .entry(injected)
          .or_insert_with(Vec::new)
          .push(node_to_range(&content));
      };
    }
    let matcher = KindMatcher::new("style_element", lang);
    for style in root.find_all(matcher) {
      let injected = find_lang(&style).unwrap_or_else(|| "css".into());
      let content = style.children().find(|c| c.kind() == "raw_text");
      if let Some(content) = content {
        map
          .entry(injected)
          .or_insert_with(Vec::new)
          .push(node_to_range(&content));
      };
    }
    map
  }
}

fn find_lang<D: Doc>(node: &Node<D>) -> Option<String> {
  let html = node.lang();
  let attr_matcher = KindMatcher::new("attribute", html.clone());
  let name_matcher = KindMatcher::new("attribute_name", html.clone());
  let val_matcher = KindMatcher::new("attribute_value", html.clone());
  node.find_all(attr_matcher).find_map(|attr| {
    let name = attr.find(&name_matcher)?;
    if name.text() != "lang" {
      return None;
    }
    let val = attr.find(&val_matcher)?;
    Some(val.text().to_string())
  })
}

fn node_to_range<D: Doc>(node: &Node<D>) -> TSRange {
  let r = node.range();
  let start = node.start_pos();
  let sp = TSPoint::new(start.0 as u32, start.1 as u32);
  let end = node.end_pos();
  let ep = TSPoint::new(end.0 as u32, end.1 as u32);
  TSRange::new(r.start as u32, r.end as u32, &sp, &ep)
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
