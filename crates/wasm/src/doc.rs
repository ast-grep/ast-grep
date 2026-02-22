use crate::ts_types as ts;
use crate::wasm_lang::{SgWasmError, WasmLang};

use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore};
use ast_grep_core::source::{Content, Doc, Edit, SgNode};
use ast_grep_core::Position;
use wasm_bindgen::prelude::*;

use std::borrow::Cow;
use std::ops::Range;

/// Rule configuration similar to YAML.
/// See https://ast-grep.github.io/reference/yaml.html
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WasmConfig {
  pub rule: serde_json::Value,
  pub constraints: Option<serde_json::Value>,
  pub language: Option<String>,
  pub transform: Option<serde_json::Value>,
  pub utils: Option<serde_json::Value>,
}

impl WasmConfig {
  pub fn parse_with(self, lang: WasmLang) -> Result<RuleCore, JsError> {
    let rule = SerializableRuleCore {
      rule: serde_json::from_value(self.rule)?,
      constraints: self.constraints.map(serde_json::from_value).transpose()?,
      transform: self.transform.map(serde_json::from_value).transpose()?,
      utils: self.utils.map(serde_json::from_value).transpose()?,
      fix: None,
    };
    let env = DeserializeEnv::new(lang);
    rule.get_matcher(env).map_err(|e| {
      let errors: Vec<_> =
        std::iter::successors(Some(&e as &dyn std::error::Error), |e| e.source())
          .map(|e| e.to_string())
          .collect();
      JsError::new(&errors.join("\n |->"))
    })
  }
}

// Content wrapper using Vec<char> encoding for WASM
#[derive(Clone)]
pub struct Wrapper {
  inner: Vec<char>,
}

impl Content for Wrapper {
  type Underlying = char;
  fn get_range(&self, range: Range<usize>) -> &[char] {
    &self.inner[range]
  }
  fn decode_str(src: &str) -> Cow<'_, [Self::Underlying]> {
    Cow::Owned(src.chars().collect())
  }
  fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<'_, str> {
    Cow::Owned(bytes.iter().collect())
  }
  fn get_char_column(&self, column: usize, _: usize) -> usize {
    column
  }
}

impl Wrapper {
  fn accept_edit(&mut self, edit: &Edit<Self>) -> ts::Edit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let input = &mut self.inner;
    let start_position = pos_for_char_offset(input, start_byte);
    let old_end_position = pos_for_char_offset(input, old_end_byte);
    input.splice(start_byte..old_end_byte, edit.inserted_text.clone());
    let new_end_position = pos_for_char_offset(input, new_end_byte);
    ts::Edit::new(
      start_byte as u32,
      old_end_byte as u32,
      new_end_byte as u32,
      &start_position,
      &old_end_position,
      &new_end_position,
    )
  }
}

fn pos_for_char_offset(input: &[char], offset: usize) -> ts::Point {
  debug_assert!(offset <= input.len());
  let (mut row, mut col) = (0, 0);
  for &c in input.iter().take(offset) {
    if '\n' == c {
      row += 1;
      col = 0;
    } else {
      col += 1;
    }
  }
  ts::Point::new(row, col)
}

// WasmDoc

#[derive(Clone)]
pub struct WasmDoc {
  lang: WasmLang,
  source: Wrapper,
  pub(crate) tree: ts::Tree,
}

impl WasmDoc {
  pub fn try_new(src: String, lang: WasmLang) -> Result<Self, SgWasmError> {
    let source = Wrapper {
      inner: src.chars().collect(),
    };
    let parser = lang.get_parser()?;
    let Some(tree) = parser.parse_with_string(&src.into(), None, None)? else {
      return Err(SgWasmError::FailedToParse);
    };
    Ok(Self { source, lang, tree })
  }
}

// Node wrapper for web-tree-sitter SyntaxNode

#[derive(Clone)]
pub struct Node(pub ts::SyntaxNode);

impl<'a> SgNode<'a> for Node {
  fn parent(&self) -> Option<Self> {
    self.0.parent().map(Node)
  }
  fn ancestors(&self, _root: Self) -> impl Iterator<Item = Self> {
    let mut parent = self.0.parent();
    std::iter::from_fn(move || {
      let inner = parent.clone()?;
      let ret = Some(Node(inner.clone()));
      parent = inner.parent();
      ret
    })
  }
  fn child(&self, nth: usize) -> Option<Self> {
    self.0.child(nth as u32).map(Node)
  }
  fn children(&self) -> impl ExactSizeIterator<Item = Self> {
    self
      .0
      .children()
      .to_vec()
      .into_iter()
      .map(|n| n.unchecked_into::<ts::SyntaxNode>())
      .map(Node)
  }
  fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
    self.0.child_for_field_id(field_id).map(Node)
  }
  fn next(&self) -> Option<Self> {
    self.0.next_sibling().map(Node)
  }
  fn prev(&self) -> Option<Self> {
    self.0.previous_sibling().map(Node)
  }
  fn is_named(&self) -> bool {
    self.0.is_named()
  }
  fn is_named_leaf(&self) -> bool {
    self.0.named_child_count() == 0
  }
  fn is_leaf(&self) -> bool {
    self.0.child_count() == 0
  }
  fn kind(&self) -> Cow<'_, str> {
    Cow::Owned(self.0.type_().into())
  }
  fn kind_id(&self) -> u16 {
    self.0.type_id()
  }
  fn node_id(&self) -> usize {
    self.0.id() as usize
  }
  fn range(&self) -> std::ops::Range<usize> {
    (self.0.start_index() as usize)..(self.0.end_index() as usize)
  }
  fn start_pos(&self) -> Position {
    let start = self.0.start_position();
    let offset = self.0.start_index();
    Position::new(
      start.row() as usize,
      start.column() as usize,
      offset as usize,
    )
  }
  fn end_pos(&self) -> Position {
    let end = self.0.end_position();
    let offset = self.0.end_index();
    Position::new(end.row() as usize, end.column() as usize, offset as usize)
  }
  fn is_missing(&self) -> bool {
    self.0.is_missing()
  }
  fn is_error(&self) -> bool {
    self.0.is_error()
  }
  fn field(&self, name: &str) -> Option<Self> {
    self.0.child_for_field_name(name).map(Node)
  }
  fn field_children(&self, field_id: Option<u16>) -> impl Iterator<Item = Self> {
    let cursor = self.0.walk();
    let has_children = cursor.goto_first_child();
    let mut done = field_id.is_none() || !has_children;
    std::iter::from_fn(move || {
      if done {
        return None;
      }
      while cursor.current_field_id() != field_id {
        if !cursor.goto_next_sibling() {
          return None;
        }
      }
      let ret = cursor.current_node();
      if !cursor.goto_next_sibling() {
        done = true;
      }
      Some(Node(ret))
    })
  }
}

impl Doc for WasmDoc {
  type Lang = WasmLang;
  type Source = Wrapper;
  type Node<'a> = Node;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.source
  }
  fn root_node(&self) -> Self::Node<'_> {
    Node(self.tree.root_node())
  }
  fn do_edit(&mut self, edit: &Edit<Self::Source>) -> Result<(), String> {
    let edit = self.source.accept_edit(edit);
    self.tree.edit(&edit);
    let parser = self.lang.get_parser().map_err(|e| e.to_string())?;
    let src = self.source.inner.iter().collect::<String>();
    let parse_ret = parser.parse_with_string(&src.into(), Some(&self.tree), None);
    let Some(tree) = parse_ret.map_err(|e| format!("{e:?}"))? else {
      return Err("Failed to parse".to_string());
    };
    self.tree = tree;
    Ok(())
  }
  fn get_node_text<'a>(&'a self, node: &Self::Node<'a>) -> Cow<'a, str> {
    Cow::Owned(node.0.text().into())
  }
}
