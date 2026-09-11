use super::pre_process_pattern;
use ast_grep_core::matcher::{KindMatcher, Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage, TSRange};
use ast_grep_core::{Doc, Language, Node};

#[derive(Clone, Copy, Debug)]
pub struct Svelte;

impl Language for Svelte {
  fn expando_char(&self) -> char {
    'z'
  }

  fn pre_process_pattern<'q>(&self, query: &'q str) -> std::borrow::Cow<'q, str> {
    pre_process_pattern(self.expando_char(), query)
  }

  fn kind_to_id(&self, kind: &str) -> u16 {
    crate::parsers::language_svelte().id_for_node_kind(kind, true)
  }

  fn field_to_id(&self, field: &str) -> Option<u16> {
    crate::parsers::language_svelte()
      .field_id_for_name(field)
      .map(|f| f.get())
  }

  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|src| StrDoc::try_new(src, *self))
  }
}

impl LanguageExt for Svelte {
  fn get_ts_language(&self) -> TSLanguage {
    crate::parsers::language_svelte()
  }

  fn injectable_languages(&self) -> Option<&'static [&'static str]> {
    Some(&["css", "js", "ts", "tsx", "scss", "less", "stylus", "coffee"])
  }

  fn extract_injections<L: LanguageExt>(
    &self,
    root: Node<StrDoc<L>>,
  ) -> Vec<(String, Vec<TSRange>)> {
    let lang = root.lang();
    let mut ret = Vec::new();
    let matcher = KindMatcher::new("script_element", lang.clone());
    for script in root.find_all(matcher) {
      let injected = find_lang(&script).unwrap_or_else(|| "js".into());
      let content = script.children().find(|c| c.kind() == "raw_text");
      if let Some(content) = content {
        ret.push((injected, vec![node_to_range(&content)]));
      }
    }
    let matcher = KindMatcher::new("style_element", lang.clone());
    for style in root.find_all(matcher) {
      let injected = find_lang(&style).unwrap_or_else(|| "css".into());
      let content = style.children().find(|c| c.kind() == "raw_text");
      if let Some(content) = content {
        ret.push((injected, vec![node_to_range(&content)]));
      }
    }
    ret
  }
}

fn find_lang<D: Doc>(node: &Node<D>) -> Option<String> {
  let lang = node.lang();
  let attr_matcher = KindMatcher::new("attribute", lang.clone());
  let name_matcher = KindMatcher::new("attribute_name", lang.clone());
  let val_matcher = KindMatcher::new("attribute_value", lang.clone());
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

  fn extract(src: &str) -> Vec<(String, Vec<TSRange>)> {
    let root = Svelte.ast_grep(src);
    Svelte.extract_injections(root.root())
  }

  #[test]
  fn test_svelte_match() {
    crate::test::test_match_lang("<div>hello</div>", "<div>hello</div>", Svelte);
    crate::test::test_match_lang(
      "<script>$$$</script>",
      "<script>const answer = 42;</script>",
      Svelte,
    );
  }

  #[test]
  fn test_svelte_extraction() {
    let entries = extract(
      r#"<script>const answer = 42;</script><script lang="ts">let value: number;</script><style>.a { color: red; }</style><style lang="scss">$color: red;</style>"#,
    );
    assert_eq!(
      entries
        .iter()
        .map(|(lang, _)| lang.as_str())
        .collect::<Vec<_>>(),
      ["js", "ts", "css", "scss"]
    );
    assert!(entries.iter().all(|(_, ranges)| ranges.len() == 1));
  }
}
