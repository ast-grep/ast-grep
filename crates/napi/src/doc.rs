use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::replacer::IndentSensitive;
use ast_grep_core::source::{Content, Doc, Edit, TSParseError};
use napi::anyhow::{anyhow, Error};
use napi_derive::napi;
use std::borrow::Cow;
use std::ops::Range;
use std::str::FromStr;
use tree_sitter::{InputEdit, Node, Parser, ParserError, Point, Tree};

#[napi]
#[derive(PartialEq)]
pub enum FrontEndLanguage {
  Html,
  JavaScript,
  Tsx,
  Css,
  TypeScript,
}

impl Language for FrontEndLanguage {
  fn get_ts_language(&self) -> TSLanguage {
    use FrontEndLanguage::*;
    match self {
      Html => tree_sitter_html::language(),
      JavaScript => tree_sitter_javascript::language(),
      TypeScript => tree_sitter_typescript::language_typescript(),
      Css => tree_sitter_css::language(),
      Tsx => tree_sitter_typescript::language_tsx(),
    }
    .into()
  }
  fn expando_char(&self) -> char {
    use FrontEndLanguage::*;
    match self {
      Css => '_',
      _ => '$',
    }
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    use FrontEndLanguage::*;
    match self {
      Css => (),
      _ => return Cow::Borrowed(query),
    }
    // use stack buffer to reduce allocation
    let mut buf = [0; 4];
    let expando = self.expando_char().encode_utf8(&mut buf);
    // TODO: use more precise replacement
    let replaced = query.replace(self.meta_var_char(), expando);
    Cow::Owned(replaced)
  }
}

impl FrontEndLanguage {
  pub const fn all_langs() -> &'static [FrontEndLanguage] {
    use FrontEndLanguage::*;
    &[Html, JavaScript, Tsx, Css, TypeScript]
  }
}

const fn alias(lang: &FrontEndLanguage) -> &[&str] {
  use FrontEndLanguage::*;
  match lang {
    Css => &["css"],
    Html => &["html"],
    JavaScript => &["javascript", "js", "jsx"],
    TypeScript => &["ts", "typescript"],
    Tsx => &["tsx"],
  }
}

impl FromStr for FrontEndLanguage {
  type Err = Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    for lang in Self::all_langs() {
      for moniker in alias(lang) {
        if s.eq_ignore_ascii_case(moniker) {
          return Ok(*lang);
        }
      }
    }
    Err(anyhow!(format!("{} is not supported in napi", s.to_string())).into())
  }
}

#[derive(Clone)]
pub struct Wrapper {
  inner: Vec<u16>,
}

impl Content for Wrapper {
  type Underlying = u16;
  fn parse_tree_sitter(
    &self,
    parser: &mut Parser,
    tree: Option<&Tree>,
  ) -> std::result::Result<Option<Tree>, ParserError> {
    parser.parse_utf16(self.inner.as_slice(), tree)
  }
  fn get_range(&self, range: Range<usize>) -> &[Self::Underlying] {
    // the range is in byte offset, but our underlying is u16
    let start = range.start / 2;
    let end = range.end / 2;
    &self.inner.as_slice()[start..end]
  }
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len() * 2;
    let input = &mut self.inner;
    let start_position = pos_for_byte_offset(input, start_byte);
    let old_end_position = pos_for_byte_offset(input, old_end_byte);
    input.splice(start_byte / 2..old_end_byte / 2, edit.inserted_text.clone());
    let new_end_position = pos_for_byte_offset(input, new_end_byte);
    InputEdit::new(
      start_byte as u32,
      old_end_byte as u32,
      new_end_byte as u32,
      &start_position,
      &old_end_position,
      &new_end_position,
    )
  }
  fn get_text<'a>(&'a self, node: &Node) -> Cow<'a, str> {
    let slice = self.inner.as_slice();
    let start = node.start_byte() as usize / 2;
    let end = node.end_byte() as usize / 2;
    String::from_utf16_lossy(&slice[start..end]).into()
  }

  fn decode_str(src: &str) -> Cow<[Self::Underlying]> {
    let v: Vec<_> = src.encode_utf16().collect();
    Cow::Owned(v)
  }

  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<str> {
    let s = String::from_utf16_lossy(bytes);
    Cow::Owned(s)
  }
}

impl IndentSensitive for Wrapper {
  const NEW_LINE: u16 = b'\n' as u16;
  const SPACE: u16 = b' ' as u16;
  const TAB: u16 = b'\t' as u16;
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
  lang: FrontEndLanguage,
  source: Wrapper,
}

impl JsDoc {
  pub fn new(src: String, lang: FrontEndLanguage) -> Self {
    let source = Wrapper {
      inner: src.encode_utf16().collect(),
    };
    Self { source, lang }
  }
}

impl Doc for JsDoc {
  type Lang = FrontEndLanguage;
  type Source = Wrapper;
  fn parse(&self, old_tree: Option<&Tree>) -> std::result::Result<Tree, TSParseError> {
    let mut parser = Parser::new()?;
    let ts_lang = self.lang.get_ts_language();
    parser.set_language(&ts_lang)?;
    if let Some(tree) = self.source.parse_tree_sitter(&mut parser, old_tree)? {
      Ok(tree)
    } else {
      Err(TSParseError::TreeUnavailable)
    }
  }
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.source
  }
  fn get_source_mut(&mut self) -> &mut Self::Source {
    &mut self.source
  }
  fn from_str(src: &str, lang: Self::Lang) -> Self {
    JsDoc::new(src.into(), lang)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::AstGrep;
  #[test]
  fn test_js_doc() {
    let doc = JsDoc::new("console.log(123)".into(), FrontEndLanguage::JavaScript);
    let grep = AstGrep::doc(doc);
    assert_eq!(grep.root().text(), "console.log(123)");
    let node = grep.root().find("console");
    assert!(node.is_some());
  }

  #[test]
  fn test_js_doc_single_node_replace() {
    let doc = JsDoc::new(
      "console.log(1 + 2 + 3)".into(),
      FrontEndLanguage::JavaScript,
    );
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
    let doc = JsDoc::new(
      "console.log(1 + 2 + 3)".into(),
      FrontEndLanguage::JavaScript,
    );
    let mut grep = AstGrep::doc(doc);
    let edit = grep
      .root()
      .replace("console.log($$$MULTI)", "log($$$MULTI)")
      .expect("should exist");
    grep.edit(edit).expect("should work");
    assert_eq!(grep.root().text(), "log(1 + 2 + 3)");
  }
}
