#![deny(clippy::all)]

// use ast_grep_config::RuleConfig;
use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::{AstGrep, NodeMatch};
use napi::bindgen_prelude::{Env, Reference, Result, SharedReference};
use napi_derive::napi;
// use serde_json::Value;

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
  pub row: u32,
  pub col: u32,
}

fn tuple_to_pos(pos: (usize, usize)) -> Pos {
  Pos {
    row: pos.0 as u32,
    col: pos.1 as u32,
  }
}

#[napi(object)]
pub struct Range {
  pub start: Pos,
  pub end: Pos,
}

#[napi(js_name = "NodeMatch")]
pub struct NodeMatchNapi {
  inner: SharedReference<AstGrepNapi, NodeMatch<'static, FrontEndLanguage>>,
}

#[napi]
impl NodeMatchNapi {
  #[napi]
  pub fn range(&self) -> Range {
    let start_pos = self.inner.start_pos();
    let end_pos = self.inner.end_pos();
    Range {
      start: tuple_to_pos(start_pos),
      end: tuple_to_pos(end_pos),
    }
  }
}

#[napi(js_name = "AstGrep")]
pub struct AstGrepNapi(AstGrep<FrontEndLanguage>);

#[napi]
impl AstGrepNapi {
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

  #[napi(factory)]
  pub fn ts(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::TypeScript)
  }

  #[napi(factory)]
  pub fn tsx(src: String) -> Self {
    Self::from_lang(src, FrontEndLanguage::Tsx)
  }

  #[napi]
  pub fn find_by_string(
    &self,
    reference: Reference<AstGrepNapi>,
    env: Env,
    pattern: String,
  ) -> Result<NodeMatchNapi> {
    Ok(NodeMatchNapi {
      inner: reference.share_with(env, |grep| Ok(grep.0.root().find(&*pattern).unwrap()))?,
    })
  }
}
