use crate::language::Language;
use std::borrow::Cow;
use tree_sitter::{Node, Parser, ParserError, Tree};

pub trait Doc: Clone {
  type Source: Content;
  type Lang: Language;
  fn get_lang(&self) -> &Self::Lang;
  fn get_source(&self) -> &Self::Source;
  fn get_source_mut(&mut self) -> &mut Self::Source;
}

#[derive(Clone)]
pub struct StrDoc<L: Language> {
  pub src: String,
  pub lang: L,
}
impl<L: Language> StrDoc<L> {
  pub fn new(src: &str, lang: L) -> Self {
    Self {
      src: src.into(),
      lang,
    }
  }
}

impl<L: Language> Doc for StrDoc<L> {
  type Source = String;
  type Lang = L;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.src
  }
  fn get_source_mut(&mut self) -> &mut Self::Source {
    &mut self.src
  }
}

pub trait Content {
  type Underlying;
  fn parse_tree_sitter(
    &self,
    parser: &mut Parser,
    tree: Option<&Tree>,
  ) -> Result<Option<Tree>, ParserError>;
  fn as_slice(&self) -> &[Self::Underlying];
  fn as_str(&self) -> &str;
  fn get_text<'a>(&'a self, node: &Node) -> Cow<'a, str>;
  /// # Safety
  /// TODO
  unsafe fn as_mut(&mut self) -> &mut Vec<u8>;
}

impl Content for String {
  type Underlying = u8;
  fn parse_tree_sitter(
    &self,
    parser: &mut Parser,
    tree: Option<&Tree>,
  ) -> Result<Option<Tree>, ParserError> {
    parser.parse(self.as_bytes(), tree)
  }
  fn as_slice(&self) -> &[Self::Underlying] {
    self.as_bytes()
  }
  fn as_str(&self) -> &str {
    self.as_str()
  }
  fn get_text<'a>(&'a self, node: &Node) -> Cow<'a, str> {
    node
      .utf8_text(self.as_bytes())
      .expect("invalid source text encoding")
  }
  unsafe fn as_mut(&mut self) -> &mut Vec<u8> {
    self.as_mut_vec()
  }
}
