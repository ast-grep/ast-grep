use ast_grep_core::{matcher::KindMatcher, AstGrep, NodeMatch, Pattern, Position};
use napi::bindgen_prelude::*;
use napi_derive::napi;

use super::NapiConfig;
use crate::{
  doc::{JsDoc, Wrapper},
  napi_lang::NapiLang,
};
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

/// Represents a node in the AST.
///
/// Each SgNode keeps the owner SgRoot alive via `Reference<SgRoot>` (NAPI
/// ref-counting) and owns its `NodeMatch` data in a `Box`.
#[napi]
pub struct SgNode {
  // `node` is dropped before `root_ref`, so node data is freed while the
  // tree is still alive.
  pub(super) node: Box<NodeMatch<'static, JsDoc>>,
  pub(super) root_ref: Reference<SgRoot>,
}

impl SgNode {
  fn to_pos(&self, pos: Position, offset: usize) -> Pos {
    Pos {
      line: pos.line() as u32,
      column: pos.column(self.node.get_node()) as u32,
      index: offset as u32 / 2,
    }
  }

  /// Create a new SgNode from a NodeMatch, cloning the root reference.
  pub(super) fn new_node(
    root_ref: &Reference<SgRoot>,
    env: Env,
    node: NodeMatch<'static, JsDoc>,
  ) -> Result<Self> {
    Ok(SgNode {
      node: Box::new(node),
      root_ref: root_ref.clone(env)?,
    })
  }
}

#[napi]
impl SgNode {
  #[napi]
  pub fn range(&self) -> Range {
    let byte_range = self.node.range();
    let start_pos = self.node.start_pos();
    let end_pos = self.node.end_pos();
    Range {
      start: self.to_pos(start_pos, byte_range.start),
      end: self.to_pos(end_pos, byte_range.end),
    }
  }

  #[napi]
  pub fn is_leaf(&self) -> bool {
    self.node.is_leaf()
  }
  #[napi]
  pub fn is_named(&self) -> bool {
    self.node.is_named()
  }
  #[napi]
  pub fn is_named_leaf(&self) -> bool {
    self.node.is_named_leaf()
  }
  /// Returns the string name of the node kind
  #[napi]
  pub fn kind(&self) -> String {
    self.node.kind().to_string()
  }
  #[napi(getter)]
  pub fn kind_to_refine(&self) -> String {
    self.node.kind().to_string()
  }
  /// Check if the node is the same kind as the given `kind` string
  #[napi]
  pub fn is(&self, kind: String) -> bool {
    self.node.kind() == kind
  }
  #[napi]
  pub fn text(&self) -> String {
    self.node.text().to_string()
  }
}

#[napi]
impl SgNode {
  #[napi]
  pub fn matches(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.node.lang();
    match m {
      Either3::A(pattern) => Ok(self.node.matches(napi_pattern(&pattern, lang)?)),
      Either3::B(kind) => Ok(self.node.matches(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.node.matches(pattern))
      }
    }
  }

  #[napi]
  pub fn inside(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.node.lang();
    match m {
      Either3::A(pattern) => Ok(self.node.inside(napi_pattern(&pattern, lang)?)),
      Either3::B(kind) => Ok(self.node.inside(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.node.inside(pattern))
      }
    }
  }

  #[napi]
  pub fn has(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.node.lang();
    match m {
      Either3::A(pattern) => Ok(self.node.has(napi_pattern(&pattern, lang)?)),
      Either3::B(kind) => Ok(self.node.has(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.node.has(pattern))
      }
    }
  }

  #[napi]
  pub fn precedes(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.node.lang();
    match m {
      Either3::A(pattern) => Ok(self.node.precedes(napi_pattern(&pattern, lang)?)),
      Either3::B(kind) => Ok(self.node.precedes(KindMatcher::from_id(kind))),
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        Ok(self.node.precedes(pattern))
      }
    }
  }

  #[napi]
  pub fn follows(&self, m: Either3<String, u16, NapiConfig>) -> Result<bool> {
    let lang = *self.node.lang();
    Ok(match m {
      Either3::A(pattern) => self.node.follows(napi_pattern(&pattern, lang)?),
      Either3::B(kind) => self.node.follows(KindMatcher::from_id(kind)),
      Either3::C(config) => self.node.follows(config.parse_with(lang)?),
    })
  }

  #[napi]
  pub fn get_match(&self, env: Env, m: String) -> Result<Option<SgNode>> {
    let node = self
      .node
      .get_env()
      .get_match(&m)
      .cloned()
      .map(NodeMatch::from);
    Self::transpose_option(&self.root_ref, env, node)
  }
  #[napi]
  pub fn get_multiple_matches(&self, env: Env, m: String) -> Result<Vec<SgNode>> {
    let nodes = self
      .node
      .get_env()
      .get_multiple_matches(&m)
      .into_iter()
      .map(NodeMatch::from);
    Self::from_iter_to_vec(&self.root_ref, env, nodes)
  }
  #[napi]
  pub fn get_transformed(&self, m: String) -> Option<String> {
    let bytes = self.node.get_env().get_transformed(&m)?;
    Some(String::from_utf16_lossy(bytes))
  }
}

/// tree traversal API
#[napi]
impl SgNode {
  /// Returns the node's SgRoot
  #[napi]
  pub fn get_root(&self, env: Env) -> Result<Reference<SgRoot>> {
    self.root_ref.clone(env)
  }
  #[napi]
  pub fn children(&self, env: Env) -> Result<Vec<SgNode>> {
    let children = self.node.children().map(NodeMatch::from);
    Self::from_iter_to_vec(&self.root_ref, env, children)
  }

  /// Returns the node's id
  #[napi]
  pub fn id(&self) -> Result<u32> {
    Ok(self.node.node_id() as u32)
  }

  #[napi]
  pub fn find(
    &self,
    env: Env,
    matcher: Either3<String, u16, NapiConfig>,
  ) -> Result<Option<SgNode>> {
    let lang = *self.node.lang();
    let node_match = match matcher {
      Either3::A(pattern) => {
        let pattern = napi_pattern(&pattern, lang)?;
        self.node.find(pattern)
      }
      Either3::B(kind) => {
        let pattern = KindMatcher::from_id(kind);
        self.node.find(pattern)
      }
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        self.node.find(pattern)
      }
    };
    Self::transpose_option(&self.root_ref, env, node_match)
  }

  fn transpose_option(
    root_ref: &Reference<SgRoot>,
    env: Env,
    node: Option<NodeMatch<'static, JsDoc>>,
  ) -> Result<Option<SgNode>> {
    if let Some(node) = node {
      Ok(Some(SgNode::new_node(root_ref, env, node)?))
    } else {
      Ok(None)
    }
  }

  #[napi]
  pub fn find_all(
    &self,
    env: Env,
    matcher: Either3<String, u16, NapiConfig>,
  ) -> Result<Vec<SgNode>> {
    let lang = *self.node.lang();
    let all_matches: Vec<_> = match matcher {
      Either3::A(pattern) => {
        let pattern = napi_pattern(&pattern, lang)?;
        self.node.find_all(pattern).collect()
      }
      Either3::B(kind) => {
        let pattern = KindMatcher::from_id(kind);
        self.node.find_all(pattern).collect()
      }
      Either3::C(config) => {
        let pattern = config.parse_with(lang)?;
        self.node.find_all(pattern).collect()
      }
    };
    Self::from_iter_to_vec(&self.root_ref, env, all_matches.into_iter())
  }

  fn from_iter_to_vec(
    root_ref: &Reference<SgRoot>,
    env: Env,
    iter: impl Iterator<Item = NodeMatch<'static, JsDoc>>,
  ) -> Result<Vec<SgNode>> {
    let mut ret = vec![];
    for node in iter {
      ret.push(SgNode::new_node(root_ref, env, node)?);
    }
    Ok(ret)
  }

  /// Finds the first child node in the `field`
  #[napi]
  pub fn field(&self, env: Env, name: String) -> Result<Option<SgNode>> {
    let node = self.node.field(&name).map(NodeMatch::from);
    Self::transpose_option(&self.root_ref, env, node)
  }

  /// Finds all the children nodes in the `field`
  #[napi]
  pub fn field_children(&self, env: Env, name: String) -> Result<Vec<SgNode>> {
    let children = self.node.field_children(&name).map(NodeMatch::from);
    Self::from_iter_to_vec(&self.root_ref, env, children)
  }

  #[napi]
  pub fn parent(&self, env: Env) -> Result<Option<SgNode>> {
    let node = self.node.parent().map(NodeMatch::from);
    Self::transpose_option(&self.root_ref, env, node)
  }

  #[napi]
  pub fn child(&self, env: Env, nth: u32) -> Result<Option<SgNode>> {
    let inner = self.node.child(nth as usize).map(NodeMatch::from);
    Self::transpose_option(&self.root_ref, env, inner)
  }

  #[napi]
  pub fn ancestors(&self, env: Env) -> Result<Vec<SgNode>> {
    let ancestors = self.node.ancestors().map(NodeMatch::from);
    Self::from_iter_to_vec(&self.root_ref, env, ancestors)
  }

  #[napi]
  pub fn next(&self, env: Env) -> Result<Option<SgNode>> {
    let inner = self.node.next().map(NodeMatch::from);
    Self::transpose_option(&self.root_ref, env, inner)
  }

  #[napi]
  pub fn next_all(&self, env: Env) -> Result<Vec<SgNode>> {
    let inner = self.node.next_all().map(NodeMatch::from);
    Self::from_iter_to_vec(&self.root_ref, env, inner)
  }

  #[napi]
  pub fn prev(&self, env: Env) -> Result<Option<SgNode>> {
    let inner = self.node.prev().map(NodeMatch::from);
    Self::transpose_option(&self.root_ref, env, inner)
  }

  #[napi]
  pub fn prev_all(&self, env: Env) -> Result<Vec<SgNode>> {
    let inner = self.node.prev_all().map(NodeMatch::from);
    Self::from_iter_to_vec(&self.root_ref, env, inner)
  }
}

/// Edit API
#[napi]
impl SgNode {
  #[napi]
  pub fn replace(&self, text: String) -> Edit {
    let byte_range = self.node.range();
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
    let offset = self.node.range().start / 2;
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

impl SgRoot {
  /// Returns a `&'static` reference to the inner `AstGrep`.
  ///
  /// # Safety
  /// The caller **must** hold a `Reference<SgRoot>` (NAPI ref-count) that
  /// keeps this `SgRoot` alive for at least as long as any value derived
  /// from the returned reference.  In practice every `SgNode` satisfies
  /// this because it stores its own `root_ref: Reference<SgRoot>`.
  pub(super) unsafe fn as_static(&self) -> &'static AstGrep<JsDoc> {
    unsafe { std::mem::transmute(&self.0) }
  }
}

#[napi]
impl SgRoot {
  /// Returns the root SgNode of the ast-grep instance.
  #[napi]
  pub fn root(&self, root_ref: Reference<SgRoot>, _env: Env) -> Result<SgNode> {
    // SAFETY: root_ref keeps self alive for at least as long as the SgNode.
    let root = unsafe { self.as_static() };
    Ok(SgNode {
      node: Box::new(root.root().into()),
      root_ref,
    })
  }
  /// Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
  /// Returns `"anonymous"` if the instance is created by `lang.parse(source)`.
  #[napi]
  pub fn filename(&self) -> Result<String> {
    Ok(self.1.clone())
  }
}

fn napi_pattern(pattern: &str, lang: NapiLang) -> Result<Pattern> {
  Pattern::try_new(pattern, lang)
    .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))
}
