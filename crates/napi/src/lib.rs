#![cfg(not(feature = "napi-noop-in-unit-test"))]

// use ast_grep_config::RuleConfig;
use ast_grep_config::{
  deserialize_rule, try_deserialize_matchers, RuleWithConstraint, SerializableMetaVarMatcher,
  SerializableRule,
};
use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::{AstGrep, KindMatcher, NodeMatch, Pattern};
use napi::bindgen_prelude::{Either3, Env, Reference, Result, SharedReference};
use napi_derive::napi;
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Clone)]
enum FrontEndLanguage {
  Html,
  JavaScript,
  Tsx,
  TypeScript,
}

impl Language for FrontEndLanguage {
  fn get_ts_language(&self) -> TSLanguage {
    use FrontEndLanguage::*;
    match self {
      Html => tree_sitter_html::language(),
      JavaScript => tree_sitter_javascript::language(),
      TypeScript => tree_sitter_typescript::language_typescript(),
      Tsx => tree_sitter_typescript::language_tsx(),
    }
    .into()
  }
}

#[napi(object)]
pub struct Pos {
  /// line number starting from 1
  pub line: u32,
  /// column number starting from 1
  pub column: u32,
  /// byte offset of the position
  pub index: u32,
}

fn to_pos(pos: (usize, usize), offset: usize) -> Pos {
  Pos {
    line: pos.0 as u32,
    column: pos.1 as u32,
    index: offset as u32,
  }
}

#[napi(object)]
pub struct Range {
  pub start: Pos,
  pub end: Pos,
}

#[napi]
pub struct SgNode {
  inner: SharedReference<SgRoot, NodeMatch<'static, FrontEndLanguage>>,
}

#[napi]
impl SgNode {
  #[napi]
  pub fn range(&self) -> Range {
    let byte_range = self.inner.range();
    let start_pos = self.inner.start_pos();
    let end_pos = self.inner.end_pos();
    Range {
      start: to_pos(start_pos, byte_range.start),
      end: to_pos(end_pos, byte_range.end),
    }
  }

  #[napi]
  pub fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }
  #[napi]
  pub fn kind(&self) -> String {
    self.inner.kind().to_string()
  }
  #[napi]
  pub fn text(&self) -> String {
    self.inner.text().to_string()
  }
}

#[napi]
impl SgNode {
  #[napi]
  pub fn matches(&self, m: String) -> bool {
    self.inner.matches(&*m)
  }

  #[napi]
  pub fn inside(&self, m: String) -> bool {
    self.inner.inside(&*m)
  }

  #[napi]
  pub fn has(&self, m: String) -> bool {
    self.inner.has(&*m)
  }

  #[napi]
  pub fn precedes(&self, m: String) -> bool {
    self.inner.precedes(&*m)
  }

  #[napi]
  pub fn follows(&self, m: String) -> bool {
    self.inner.follows(&*m)
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
}

#[napi(object)]
pub struct NapiConfig {
  pub rule: serde_json::Value,
  pub constraints: Option<serde_json::Value>,
}

fn parse_config(
  config: NapiConfig,
  lang: FrontEndLanguage,
) -> Result<RuleWithConstraint<FrontEndLanguage>> {
  let rule: SerializableRule = serde_json::from_value(config.rule)?;
  let rule = deserialize_rule(rule, lang.clone())
    .map_err(|_| napi::Error::new(napi::Status::InvalidArg, "invalid rule".to_string()))?;
  let matchers = if let Some(matchers) = config.constraints {
    let matchers: HashMap<String, SerializableMetaVarMatcher> = serde_json::from_value(matchers)?;
    try_deserialize_matchers(matchers, lang)
      .map_err(|_| napi::Error::new(napi::Status::InvalidArg, "invalid matchers".to_string()))?
  } else {
    MetaVarMatchers::default()
  };
  Ok(RuleWithConstraint { rule, matchers })
}

/// tree traversal API
#[napi]
impl SgNode {
  #[napi]
  pub fn children(&self, reference: Reference<SgNode>, env: Env) -> Result<Vec<SgNode>> {
    let children = reference.inner.children().map(NodeMatch::from);
    Self::from_iter_to_vec(&reference, env, children)
  }

  #[napi]
  pub fn find(
    &self,
    reference: Reference<SgNode>,
    env: Env,
    matcher: Either3<String, u16, NapiConfig>,
  ) -> Result<Option<SgNode>> {
    let lang = reference.inner.lang().clone();
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
        let pattern = parse_config(config, lang)?;
        reference.inner.find(pattern)
      }
    };
    Self::transpose_option(reference, env, node_match)
  }

  fn transpose_option(
    reference: Reference<SgNode>,
    env: Env,
    node: Option<NodeMatch<'static, FrontEndLanguage>>,
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
    let lang = reference.inner.lang().clone();
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
        let pattern = parse_config(config, lang)?;
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
    iter: impl Iterator<Item = NodeMatch<'static, FrontEndLanguage>>,
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

#[napi(js_name = "AstGrep")]
pub struct SgRoot(AstGrep<FrontEndLanguage>);

#[napi]
impl SgRoot {
  fn from_lang(src: String, fe_lang: FrontEndLanguage) -> Self {
    Self(AstGrep::new(src, fe_lang))
  }

  #[napi(factory)]
  pub fn html(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::Html)
  }

  #[napi(factory)]
  pub fn js(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::JavaScript)
  }

  /// Synonymn for js
  #[napi(factory)]
  pub fn jsx(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::JavaScript)
  }

  #[napi(factory)]
  pub fn ts(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::TypeScript)
  }

  #[napi(factory)]
  pub fn tsx(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::Tsx)
  }

  #[napi]
  pub fn root(&self, root_ref: Reference<SgRoot>, env: Env) -> Result<SgNode> {
    let inner = root_ref.share_with(env, |root| Ok(root.0.root().into()))?;
    Ok(SgNode { inner })
  }
}
