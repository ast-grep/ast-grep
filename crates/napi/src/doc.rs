use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::source::{Content, Doc, Edit, TSParseError};
use napi::bindgen_prelude::*;
use napi::JsStringUtf16;
use napi_derive::napi;
use std::borrow::Cow;
use tree_sitter::{InputEdit, Node, Parser, ParserError, Point, Tree};

#[napi]
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

pub struct Wrapper {
  inner: JsStringUtf16,
  env: Env,
}
impl Clone for Wrapper {
  fn clone(&self) -> Self {
    let s = self.env.create_string_utf16(self.inner.as_slice()).unwrap();
    let inner = s.into_utf16().unwrap();
    Self {
      inner,
      env: self.env.clone(),
    }
  }
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
  fn as_slice(&self) -> &[Self::Underlying] {
    self.inner.as_slice()
  }
  fn transform_str(s: &str) -> Vec<Self::Underlying> {
    s.encode_utf16().collect()
  }
  fn accept_edit(&mut self, edit: &Edit<Self>) -> InputEdit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let mut input = self.inner.to_vec();
    let start_position = position_for_offset(&input, start_byte);
    let old_end_position = position_for_offset(&input, old_end_byte);
    input.splice(start_byte..old_end_byte, edit.inserted_text.clone());
    let new_end_position = position_for_offset(&input, new_end_byte);
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
    String::from_utf16_lossy(node.utf16_text(slice)).into()
  }
}

fn position_for_offset(input: &[u16], offset: usize) -> Point {
  debug_assert!(offset <= input.len());
  let (mut row, mut col) = (0, 0);
  for c in char::decode_utf16(input.iter().copied()) {
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
}
