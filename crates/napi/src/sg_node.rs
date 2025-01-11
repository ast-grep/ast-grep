use ast_grep_core::{matcher::KindMatcher, AstGrep, NodeMatch, Pattern, Position};
use napi::bindgen_prelude::*;
use napi_derive::napi;

use super::NapiConfig;
use crate::doc::{JsDoc, Wrapper};
use ast_grep_core::source::Content;

#[napi(object)]
pub struct Edit {
  /// The start position of the edit
  pub start_pos: u32,
  /// The end position of the edit
  pub end_pos: u32,
  /// The text to be inserted
  pub inserted_text: String,
}

#[napi(object)]
pub struct Pos {
  /// line number starting from 0
  pub line: u32,
  /// column number starting from 0
  pub column: u32,
  /// byte offset of the position
  pub index: u32,
}

#[napi(object)]
pub struct Range {
  /// starting position of the range
  pub start: Pos,
  /// ending position of the range
  pub end: Pos,
}

#[napi]
pub struct SgNode {
  pub(super) inner: SharedReference<SgRoot, NodeMatch<'static, JsDoc>>,
}

impl SgNode {
  fn to_pos(&self, pos: Position, offset: usize) -> Pos {
    Pos {
      line: pos.line() as u32,
      column: pos.column(&self.inner) as u32,
      index: offset as u32 / 2,
    }
  }
}

#[napi]
impl SgNode {
  #[napi]
  pub fn range(&self) -> Range {
    let byte_range = self.inner.range();
    let start_pos = self.inner.start_pos();
    let end_pos = self.inner.end_pos();
    Range {
      start: self.to_pos(start_pos, byte_range.start),
      end: self.to_pos(end_pos, byte_range.end),
    }
  }

  #[napi]
  pub fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }
  #[napi]
  pub fn is_named(&self) -> bool {
    self.inner.is_named()
  }
  #[napi]
  pub fn is_named_leaf(&self) -> bool {
    self.inner.is_named_leaf()
  }
  /// Returns the string name of the node kind
  #[napi]
  pub fn kind(&self) -> String {
    self.inner.kind().to_string()
  }
  #[napi(getter)]
  pub fn kind_to_refine(&self) -> String {
    self.inner.kind().to_string()
  }
  /// Check if the node is the same kind as the given `kind` string
  #[napi]
  pub fn is(&self, kind: String) -> bool {
    self.inner.kind() == kind
  }
  #[napi]
  pub fn text(&self) -> String {
    self.inner.text().to_string()
  }
}

#[napi]
impl SgNode {
  #[napi]
  pub fn matches(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.inner.lang();
    match m {
      Either3::A(pattern) => Ok(self.inner.matches(Pattern::new(&pattern, lang))),
      Either3::B(kind) => Ok(self.inner.matches(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.inner.matches(pattern))
      }
    }
  }

  #[napi]
  pub fn inside(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.inner.lang();
    match m {
      Either3::A(pattern) => Ok(self.inner.inside(Pattern::new(&pattern, lang))),
      Either3::B(kind) => Ok(self.inner.inside(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.inner.inside(pattern))
      }
    }
  }

  #[napi]
  pub fn has(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.inner.lang();
    match m {
      Either3::A(pattern) => Ok(self.inner.has(Pattern::new(&pattern, lang))),
      Either3::B(kind) => Ok(self.inner.has(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.inner.has(pattern))
      }
    }
  }

  #[napi]
  pub fn precedes(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.inner.lang();
    match m {
      Either3::A(pattern) => Ok(self.inner.precedes(Pattern::new(&pattern, lang))),
      Either3::B(kind) => Ok(self.inner.precedes(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.inner.precedes(pattern))
      }
    }
  }

  #[napi]
  pub fn follows(&self, m: Either3<String, u16, NapiConfig>) -> bool {
    let lang = *self.inner.lang();
    match m {
      Either3::A(pattern) => self.inner.follows(Pattern::new(&pattern, lang)),
      Either3::B(kind) => self.inner.follows(KindMatcher::from_id(kind)),
      Either3::C(config) => self.inner.follows(config.parse_with(lang).unwrap()),
    }
  }

  #[napi]
  pub fn get_match(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    m: String,
  ) -> Result<Option<SgNode>> {
    let node = self
      .inner
      .get_env()
      .get_match(&m)
      .cloned()
      .map(NodeMatch::from);
    Self::transpose_option(reference, env, node)
  }
  #[napi]
  pub fn get_multiple_matches(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    m: String,
  ) -> Result<Vec<SgNode>> {
    let nodes = self
      .inner
      .get_env()
      .get_multiple_matches(&m)
      .into_iter()
      .map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, nodes)
  }
  #[napi]
  pub fn get_transformed(&self, m: String) -> Option<String> {
    let bytes = self.inner.get_env().get_transformed(&m)?;
    Some(String::from_utf16_lossy(bytes))
  }
}

/// tree traversal API
#[napi]
impl SgNode {
  /// Returns the node's SgRoot
  #[napi]
  pub fn get_root(&self, _: Reference<SgNode>, env: Env) -> Result<Reference<SgRoot>> {
    let root = self.inner.clone_owner(env)?;
    Ok(root)
  }
  #[napi]
  pub fn children(&self, reference: Reference<SgNode>, env: Env) -> Result<Vec<SgNode>> {
    let children = reference.inner.children().map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, children)
  }

  /// Returns the node's id
  #[napi]
  pub fn id(&self) -> Result<u32> {
    Ok(self.inner.node_id() as u32)
  }

  #[napi]
  pub fn find(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    matcher: Either3<String, u16, NapiConfig>,
  ) -> Result<Option<SgNode>> {
    let lang = *reference.inner.lang();
    let node_match = match matcher {
      Either3::A(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        reference.inner.find(pattern)
      }
      Either3::B(kind) => {
        let pattern = KindMatcher::from_id(kind);
        reference.inner.find(pattern)
      }
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        reference.inner.find(pattern)
      }
    };
    Self::transpose_option(reference, env, node_match)
  }

  fn transpose_option(
    reference: Reference<SgNode>,
    env: Env,
    node: Option<NodeMatch<'static, JsDoc>>,
  ) -> Result<Option<SgNode>> {
    if let Some(node) = node {
      let root_ref = reference.inner.clone_owner(env)?;
      let inner = root_ref.share_with(env, move |_| Ok(node))?;
      Ok(Some(SgNode { inner }))
    } else {
      Ok(None)
    }
  }

  #[napi]
  pub fn find_all(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    matcher: Either3<String, u16, NapiConfig>,
  ) -> Result<Vec<SgNode>> {
    let mut ret = vec![];
    let lang = *reference.inner.lang();
    let all_matches: Vec<_> = match matcher {
      Either3::A(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        reference.inner.find_all(pattern).collect()
      }
      Either3::B(kind) => {
        let pattern = KindMatcher::from_id(kind);
        reference.inner.find_all(pattern).collect()
      }
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        reference.inner.find_all(pattern).collect()
      }
    };
    for node_match in all_matches {
      let root_ref = reference.inner.clone_owner(env)?;
      let sg_node = SgNode {
        inner: root_ref.share_with(env, move |_| Ok(node_match))?,
      };
      ret.push(sg_node);
    }
    Ok(ret)
  }

  fn from_iter_to_vec(
    reference: &Reference<SgNode>,
    env: Env,
    iter: impl Iterator<Item = NodeMatch<'static, JsDoc>>,
  ) -> Result<Vec<SgNode>> {
    let mut ret = vec![];
    for node in iter {
      let root_ref = reference.inner.clone_owner(env)?;
      let sg_node = SgNode {
        inner: root_ref.share_with(env, move |_| Ok(node))?,
      };
      ret.push(sg_node);
    }
    Ok(ret)
  }

  /// Finds the first child node in the `field`
  #[napi]
  pub fn field(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    name: String,
  ) -> Result<Option<SgNode>> {
    let node = reference.inner.field(&name).map(NodeMatch::from);
    Self::transpose_option(reference, env, node)
  }

  /// Finds all the children nodes in the `field`
  #[napi]
  pub fn field_children(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    name: String,
  ) -> Result<Vec<SgNode>> {
    let children = reference.inner.field_children(&name).map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, children)
  }

  #[napi]
  pub fn parent(&self, reference: Reference<SgNode>, env: Env) -> Result<Option<SgNode>> {
    let node = reference.inner.parent().map(NodeMatch::from);
    Self::transpose_option(reference, env, node)
  }

  #[napi]
  pub fn child(&self, reference: Reference<SgNode>, env: Env, nth: u32) -> Result<Option<SgNode>> {
    let inner = reference.inner.child(nth as usize).map(NodeMatch::from);
    Self::transpose_option(reference, env, inner)
  }

  #[napi]
  pub fn ancestors(&self, reference: Reference<SgNode>, env: Env) -> Result<Vec<SgNode>> {
    let ancestors = reference.inner.ancestors().map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, ancestors)
  }

  #[napi]
  pub fn next(&self, reference: Reference<SgNode>, env: Env) -> Result<Option<SgNode>> {
    let inner = reference.inner.next().map(NodeMatch::from);
    Self::transpose_option(reference, env, inner)
  }

  #[napi]
  pub fn next_all(&self, reference: Reference<SgNode>, env: Env) -> Result<Vec<SgNode>> {
    let inner = reference.inner.next_all().map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, inner)
  }

  #[napi]
  pub fn prev(&self, reference: Reference<SgNode>, env: Env) -> Result<Option<SgNode>> {
    let inner = reference.inner.prev().map(NodeMatch::from);
    Self::transpose_option(reference, env, inner)
  }

  #[napi]
  pub fn prev_all(&self, reference: Reference<SgNode>, env: Env) -> Result<Vec<SgNode>> {
    let inner = reference.inner.prev_all().map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, inner)
  }
}

/// Edit API
#[napi]
impl SgNode {
  #[napi]
  pub fn replace(&self, text: String) -> Edit {
    let byte_range = self.inner.range();
    // the text is u16, need to convert to JS str length
    Edit {
      start_pos: (byte_range.start / 2) as u32,
      end_pos: (byte_range.end / 2) as u32,
      inserted_text: text,
    }
  }

  #[napi]
  pub fn commit_edits(&self, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|edit| edit.start_pos);
    let mut new_content = Vec::new();
    let text = self.text();
    let old_content = Wrapper::decode_str(&text);
    let offset = self.inner.range().start / 2;
    let mut start = 0;
    for diff in edits {
      let pos = diff.start_pos as usize - offset;
      // skip overlapping edits
      if start > pos {
        continue;
      }
      new_content.extend(&old_content[start..pos]);
      let bytes = Wrapper::decode_str(&diff.inserted_text);
      new_content.extend(&*bytes);
      start = diff.end_pos as usize - offset;
    }
    // add trailing statements
    new_content.extend(&old_content[start..]);
    Wrapper::encode_bytes(&new_content).to_string()
  }
}

/// Represents the parsed tree of code.
#[napi]
pub struct SgRoot(pub(super) AstGrep<JsDoc>, pub(super) String);

#[napi]
impl SgRoot {
  /// Returns the root SgNode of the ast-grep instance.
  #[napi]
  pub fn root(&self, root_ref: Reference<SgRoot>, env: Env) -> Result<SgNode> {
    let inner = root_ref.share_with(env, |root| Ok(root.0.root().into()))?;
    Ok(SgNode { inner })
  }
  /// Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
  /// Returns `"anonymous"` if the instance is created by `lang.parse(source)`.
  #[napi]
  pub fn filename(&self) -> Result<String> {
    Ok(self.1.clone())
  }
}
