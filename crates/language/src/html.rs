use super::pre_process_pattern;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage, TSRange};
use ast_grep_core::Language;
use ast_grep_core::{matcher::KindMatcher, Doc, Node};
use std::collections::HashMap;

// tree-sitter-html uses locale dependent iswalnum for tagName
// https://github.com/tree-sitter/tree-sitter-html/blob/b5d9758e22b4d3d25704b72526670759a9e4d195/src/scanner.c#L194
#[derive(Clone, Copy, Debug)]
pub struct Html;
impl Language for Html {
  fn expando_char(&self) -> char {
    'z'
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> std::borrow::Cow<'q, str> {
    pre_process_pattern(self.expando_char(), query)
  }
  fn kind_to_id(&self, kind: &str) -> u16 {
    crate::parsers::language_html().id_for_node_kind(kind, true)
  }
  fn field_to_id(&self, field: &str) -> Option<u16> {
    crate::parsers::language_html()
      .field_id_for_name(field)
      .map(|f| f.get())
  }
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|src| StrDoc::try_new(src, *self))
  }
}
impl LanguageExt for Html {
  fn get_ts_language(&self) -> TSLanguage {
    crate::parsers::language_html()
  }
  fn injectable_languages(&self) -> Option<&'static [&'static str]> {
    Some(&["css", "js", "ts", "tsx", "scss", "less", "stylus", "coffee"])
  }
  fn extract_injections<L: LanguageExt>(
    &self,
    root: Node<StrDoc<L>>,
  ) -> HashMap<String, Vec<TSRange>> {
    let lang = root.lang();
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
    let matcher = KindMatcher::new("style_element", lang.clone());
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
  let sp = start.byte_point();
  let sp = tree_sitter::Point::new(sp.0, sp.1);
  let end = node.end_pos();
  let ep = end.byte_point();
  let ep = tree_sitter::Point::new(ep.0, ep.1);
  TSRange {
    start_byte: r.start,
    end_byte: r.end,
    start_point: sp,
    end_point: ep,
  }
}

#[cfg(test)]
mod test {
  use super::*;

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

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
    use crate::test::test_replace_lang;
    test_replace_lang(src, pattern, replacer, Html)
  }

  #[test]
  fn test_html_replace() {
    let ret = test_replace(
      r#"<div class='foo'>bar</div>"#,
      r#"<$TAG class='foo'>$$$B</$TAG>"#,
      r#"<$TAG class='$$$B'>foo</$TAG>"#,
    );
    assert_eq!(ret, r#"<div class='bar'>foo</div>"#);
  }

  fn extract(src: &str) -> HashMap<String, Vec<TSRange>> {
    let root = Html.ast_grep(src);
    Html.extract_injections(root.root())
  }

  #[test]
  fn test_html_extraction() {
    let map = extract("<script>a</script><style>.a{}</style>");
    assert!(map.contains_key("css"));
    assert!(map.contains_key("js"));
    assert_eq!(map["css"].len(), 1);
    assert_eq!(map["js"].len(), 1);
  }

  #[test]
  fn test_explicit_lang() {
    let map = extract("<script lang='ts'>a</script><script lang=ts>.a{}</script><style lang=scss></style><style lang=\"scss\"></style>");
    assert!(map.contains_key("ts"));
    assert_eq!(map["ts"].len(), 2);
    assert_eq!(map["scss"].len(), 2);
  }
}
