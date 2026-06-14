use crate::napi_lang::NapiLang;

use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore};
use ast_grep_core::source::{Content, Doc, Edit};
use ast_grep_core::tree_sitter::{ContentExt, LanguageExt, TSParseError};
use napi::anyhow::Error;
use napi::bindgen_prelude::Result as NapiResult;
use napi_derive::napi;
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

use std::borrow::Cow;
use std::ops::Range;

/// Rule configuration similar to YAML
/// See https://ast-grep.github.io/reference/yaml.html
#[napi(object)]
pub struct NapiConfig {
  /// The rule object, see https://ast-grep.github.io/reference/rule.html
  pub rule: serde_json::Value,
  /// See https://ast-grep.github.io/guide/rule-config.html#constraints
  pub constraints: Option<serde_json::Value>,
  /// Available languages: html, css, js, jsx, ts, tsx
  pub language: Option<String>,
  /// transform is NOT useful in JavaScript. You can use JS code to directly transform the result.
  /// https://ast-grep.github.io/reference/yaml.html#transform
  pub transform: Option<serde_json::Value>,
  /// https://ast-grep.github.io/guide/rule-config/utility-rule.html
  pub utils: Option<serde_json::Value>,
}

impl NapiConfig {
  pub fn parse_with(self, lang: NapiLang) -> NapiResult<RuleCore> {
    let rule = SerializableRuleCore {
      rule: serde_json::from_value(self.rule)?,
      constraints: self.constraints.map(serde_json::from_value).transpose()?,
      transform: self.transform.map(serde_json::from_value).transpose()?,
      utils: self.utils.map(serde_json::from_value).transpose()?,
      fix: None,
    };
    let env = DeserializeEnv::new(lang);
    rule.get_matcher(env).map_err(|e| {
      let error = Error::from(e)
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
      napi::Error::new(napi::Status::InvalidArg, error.join("\n |->"))
    })
  }
}

#[derive(Clone)]
pub struct Wrapper {
  inner: Vec<u16>,
}

impl Content for Wrapper {
  type Underlying = u16;
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying] {
    // the range is in byte offset, but our underlying is u16
    let start = range.start / 2;
    let end = range.end / 2;
    &self.inner.as_slice()[start..end]
  }
  fn decode_str(src: &str) -> Cow<'_, [Self::Underlying]> {
    let v: Vec<_> = src.encode_utf16().collect();
    Cow::Owned(v)
  }

  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<'_, str> {
    let s = String::from_utf16_lossy(bytes);
    Cow::Owned(s)
  }
  fn get_char_column(&self, column: usize, _offset: usize) -> usize {
    // utf-16 is 2 bytes per character, this is O(1) operation!
    column / 2
  }
}

impl ContentExt for Wrapper {
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len() * 2;
    let input = &mut self.inner;
    let start_position = pos_for_byte_offset(input, start_byte);
    let old_end_position = pos_for_byte_offset(input, old_end_byte);
    input.splice(start_byte / 2..old_end_byte / 2, edit.inserted_text.clone());
    let new_end_position = pos_for_byte_offset(input, new_end_byte);
    InputEdit {
      start_byte,
      old_end_byte,
      new_end_byte,
      start_position,
      old_end_position,
      new_end_position,
    }
  }
}

fn pos_for_byte_offset(input: &[u16], byte_offset: usize) -> Point {
  let offset = byte_offset / 2;
  debug_assert!(offset <= input.len());
  let (mut row, mut col) = (0, 0);
  for c in char::decode_utf16(input.iter().copied()).take(offset) {
    if let Ok('\n') = c {
      row += 1;
      col = 0;
    } else {
      col += 1;
    }
  }
  Point::new(row, col)
}

#[derive(Clone)]
pub struct JsDoc {
  lang: NapiLang,
  source: Wrapper,
  tree: tree_sitter::Tree,
}

fn parse(
  source: &Wrapper,
  lang: &NapiLang,
  old_tree: Option<&Tree>,
) -> std::result::Result<Tree, TSParseError> {
  let mut parser = Parser::new();
  let ts_lang = lang.get_ts_language();
  parser.set_language(&ts_lang)?;
  if let Some(tree) = parser.parse_utf16_le(source.inner.as_slice(), old_tree) {
    Ok(tree)
  } else {
    Err(TSParseError::TreeUnavailable)
  }
}

impl JsDoc {
  pub fn try_new(src: String, lang: NapiLang) -> napi::anyhow::Result<Self> {
    let source = Wrapper {
      inner: src.encode_utf16().collect(),
    };
    let tree = parse(&source, &lang, None)?;
    Ok(Self { source, lang, tree })
  }
}

impl Doc for JsDoc {
  type Lang = NapiLang;
  type Source = Wrapper;
  type Node<'r> = Node<'r>;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.source
  }
  fn do_edit(&mut self, edit: &Edit<Self::Source>) -> Result<(), String> {
    let source = &mut self.source;
    let input_edit = source.accept_edit(edit);
    self.tree.edit(&input_edit);
    self.tree = parse(source, &self.lang, Some(&self.tree)).map_err(|e| e.to_string())?;
    Ok(())
  }
  fn root_node(&self) -> Node<'_> {
    self.tree.root_node()
  }
  fn get_node_text<'a>(&'a self, node: &Node) -> Cow<'a, str> {
    let slice = self.source.inner.as_slice();
    let start = node.start_byte() / 2;
    let end = node.end_byte() / 2;
    String::from_utf16_lossy(&slice[start..end]).into()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::AstGrep;
  use ast_grep_language::SupportLang;
  fn make_doc(src: &str) -> JsDoc {
    JsDoc::try_new(src.to_string(), SupportLang::JavaScript.into()).expect("should work")
  }
  #[test]
  fn test_js_doc() {
    let doc = make_doc("console.log(123)");
    let grep = AstGrep::doc(doc);
    assert_eq!(grep.root().text(), "console.log(123)");
    let node = grep.root().find("console");
    assert!(node.is_some());
  }

  #[test]
  fn test_js_doc_single_node_replace() {
    let doc = make_doc("console.log(1 + 2 + 3)");
    let mut grep = AstGrep::doc(doc);
    let edit = grep
      .root()
      .replace("console.log($SINGLE)", "log($SINGLE)")
      .expect("should exist");
    grep.edit(edit).expect("should work");
    assert_eq!(grep.root().text(), "log(1 + 2 + 3)");
  }

  #[test]
  fn test_js_doc_multiple_node_replace() {
    let doc = make_doc("console.log(1 + 2 + 3)");
    let mut grep = AstGrep::doc(doc);
    let edit = grep
      .root()
      .replace("console.log($$$MULTI)", "log($$$MULTI)")
      .expect("should exist");
    grep.edit(edit).expect("should work");
    assert_eq!(grep.root().text(), "log(1 + 2 + 3)");
  }
}
