#![cfg(not(feature = "napi-noop-in-unit-test"))]

// use ast_grep_config::RuleConfig;
use ast_grep_config::{
  deserialize_rule, try_deserialize_matchers, DeserializeEnv, RuleWithConstraint,
  SerializableMetaVarMatcher, SerializableRule,
};
use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::pinned::{NodeData, PinnedNodeData};
use ast_grep_core::{matcher::KindMatcher, AstGrep, NodeMatch, Pattern};
use ignore::types::TypesBuilder;
use ignore::{WalkBuilder, WalkState};
use napi::anyhow::{anyhow, Context, Result as Ret};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{JsNumber, Task};
use napi_derive::napi;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::channel;

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
  pub language: Option<FrontEndLanguage>,
}

fn parse_config(
  config: NapiConfig,
  language: FrontEndLanguage,
) -> Result<RuleWithConstraint<FrontEndLanguage>> {
  let lang = config.language.unwrap_or(language);
  let rule: SerializableRule = serde_json::from_value(config.rule)?;
  let rule = deserialize_rule(rule, &DeserializeEnv::new(lang))
    .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
  let matchers = if let Some(matchers) = config.constraints {
    let matchers: HashMap<String, SerializableMetaVarMatcher> = serde_json::from_value(matchers)?;
    try_deserialize_matchers(matchers, lang)
      .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?
  } else {
    MetaVarMatchers::default()
  };
  Ok(RuleWithConstraint::new(rule).with_matchers(matchers))
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

#[napi]
pub struct SgRoot(AstGrep<FrontEndLanguage>, String);

#[napi]
impl SgRoot {
  #[napi]
  pub fn root(&self, root_ref: Reference<SgRoot>, env: Env) -> Result<SgNode> {
    let inner = root_ref.share_with(env, |root| Ok(root.0.root().into()))?;
    Ok(SgNode { inner })
  }
  #[napi]
  pub fn filename(&self) -> Result<String> {
    Ok(self.1.clone())
  }
}

macro_rules! impl_lang_mod {
    ($name: ident, $lang: ident) =>  {
      #[napi]
      pub mod $name {
        use super::*;
        use super::FrontEndLanguage::*;
        #[napi]
        pub fn parse(src: String) -> SgRoot {
          SgRoot(AstGrep::new(src, $lang), "anonymous".into())
        }
        #[napi]
        pub fn kind(kind_name: String) -> u16 {
          $lang.get_ts_language().id_for_node_kind(&kind_name, /* named */ true)
        }
        #[napi]
        pub fn pattern(pattern: String) -> NapiConfig {
          NapiConfig {
            rule: serde_json::json!({
              "pattern": pattern,
            }),
            constraints: None,
            language: Some($lang),
          }
        }
        #[napi(
          ts_args_type = "config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void",
          ts_return_type = "Promise<number>"
        )]
        pub fn find_in_files(config: FindConfig, callback: JsFunction) -> Result<AsyncTask<FindInFiles>> {
          find_in_files_impl($lang, config, callback)
        }
      }
    }
}

impl_lang_mod!(html, Html);
impl_lang_mod!(js, JavaScript);
impl_lang_mod!(jsx, JavaScript);
impl_lang_mod!(ts, TypeScript);
impl_lang_mod!(tsx, Tsx);

pub struct IterateFiles<D> {
  paths: Vec<String>,
  tsfn: D,
  producer: fn(&D, std::result::Result<ignore::DirEntry, ignore::Error>) -> Ret<bool>,
}

impl<T: 'static + Send + Sync> Task for IterateFiles<T> {
  type Output = u32;
  type JsValue = JsNumber;

  fn compute(&mut self) -> Result<Self::Output> {
    if self.paths.is_empty() {
      return Err(anyhow!("paths cannot be empty.").into());
    }
    let types = TypesBuilder::new()
      .add_defaults()
      .select("css")
      .select("html")
      .select("js")
      .select("ts")
      .build()
      .unwrap();
    let tsfn = &self.tsfn;
    let mut paths = self.paths.drain(..);
    let mut builder = WalkBuilder::new(paths.next().unwrap());
    for path in paths {
      builder.add(path);
    }
    let file_count = AtomicU32::new(0);
    let (tx, rx) = channel();
    let walker = builder.types(types).build_parallel();
    let producer = self.producer;
    walker.run(|| {
      let tx = tx.clone();
      let file_count = &file_count;
      Box::new(move |entry| match producer(tsfn, entry) {
        Ok(true) => {
          // file is sent to JS thread, increment file count
          if tx.send(()).is_ok() {
            file_count.fetch_add(1, Ordering::AcqRel);
            WalkState::Continue
          } else {
            WalkState::Quit
          }
        }
        Ok(false) => WalkState::Continue,
        Err(_) => WalkState::Skip,
      })
    });
    // Drop the last sender to stop `rx` waiting for message.
    // The program will not complete if we comment this out.
    drop(tx);
    while rx.recv().is_ok() {
      // pass
    }
    Ok(file_count.load(Ordering::Acquire))
  }
  fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
    env.create_uint32(output)
  }
}

// See https://github.com/ast-grep/ast-grep/issues/206
// NodeJS has a 1000 file limitation on sync iteration count.
// https://github.com/nodejs/node/blob/8ba54e50496a6a5c21d93133df60a9f7cb6c46ce/src/node_api.cc#L336
const THREAD_FUNC_QUEUE_SIZE: usize = 1000;

type ParseFiles = IterateFiles<ThreadsafeFunction<SgRoot, ErrorStrategy::CalleeHandled>>;

#[napi(
  ts_args_type = "paths: string[], callback: (err: null | Error, result: SgRoot) => void",
  ts_return_type = "Promise<number>"
)]
pub fn parse_files(paths: Vec<String>, callback: JsFunction) -> Result<AsyncTask<ParseFiles>> {
  let tsfn: ThreadsafeFunction<SgRoot, ErrorStrategy::CalleeHandled> =
    callback.create_threadsafe_function(THREAD_FUNC_QUEUE_SIZE, |ctx| Ok(vec![ctx.value]))?;
  Ok(AsyncTask::new(ParseFiles {
    paths,
    tsfn,
    producer: call_sg_root,
  }))
}

// returns if the entry is a file and sent to JavaScript queue
fn call_sg_root(
  tsfn: &ThreadsafeFunction<SgRoot, ErrorStrategy::CalleeHandled>,
  entry: std::result::Result<ignore::DirEntry, ignore::Error>,
) -> Ret<bool> {
  let entry = entry?;
  if !entry
    .file_type()
    .context("could not use stdin as file")?
    .is_file()
  {
    return Ok(false);
  }
  let (root, path) = get_root(entry)?;
  let sg = SgRoot(root, path);
  tsfn.call(Ok(sg), ThreadsafeFunctionCallMode::Blocking);
  Ok(true)
}

fn get_root(entry: ignore::DirEntry) -> Ret<(AstGrep<FrontEndLanguage>, String)> {
  use FrontEndLanguage::*;
  let path = entry.into_path();
  let file_content = std::fs::read_to_string(&path)?;
  let ext = path
    .extension()
    .context("check file")?
    .to_str()
    .context("to str")?;
  let lang = match ext {
    "css" | "scss" => Css,
    "html" | "htm" | "xhtml" => Html,
    "cjs" | "js" | "mjs" | "jsx" => JavaScript,
    "ts" => TypeScript,
    "tsx" => Tsx,
    _ => return Err(anyhow!("file not recognized")),
  };
  Ok((lang.ast_grep(file_content), path.to_string_lossy().into()))
}

type FindInFiles = IterateFiles<(
  ThreadsafeFunction<PinnedNodes, ErrorStrategy::CalleeHandled>,
  RuleWithConstraint<FrontEndLanguage>,
)>;

pub struct PinnedNodes(
  PinnedNodeData<FrontEndLanguage, Vec<NodeMatch<'static, FrontEndLanguage>>>,
  String,
);
unsafe impl Send for PinnedNodes {}
unsafe impl Sync for PinnedNodes {}

#[napi(object)]
pub struct FindConfig {
  pub paths: Vec<String>,
  pub matcher: NapiConfig,
}

fn find_in_files_impl(
  lang: FrontEndLanguage,
  config: FindConfig,
  callback: JsFunction,
) -> Result<AsyncTask<FindInFiles>> {
  let tsfn = callback.create_threadsafe_function(THREAD_FUNC_QUEUE_SIZE, |ctx| {
    from_pinned_data(ctx.value, ctx.env)
  })?;
  let rule = parse_config(config.matcher, lang)?;
  Ok(AsyncTask::new(FindInFiles {
    paths: config.paths,
    tsfn: (tsfn, rule),
    producer: call_sg_node,
  }))
}

// TODO: optimize
fn from_pinned_data(pinned: PinnedNodes, env: napi::Env) -> Result<Vec<Vec<SgNode>>> {
  let (root, nodes) = pinned.0.into_raw();
  let sg_root = SgRoot(AstGrep { inner: root }, pinned.1);
  let reference = SgRoot::into_reference(sg_root, env)?;
  let mut v = vec![];
  for mut node in nodes {
    let root_ref = reference.clone(env)?;
    let sg_node = SgNode {
      inner: root_ref.share_with(env, |root| {
        let r = &root.0.inner;
        node.visit_nodes(|n| unsafe { r.readopt(n) });
        Ok(node)
      })?,
    };
    v.push(sg_node);
  }
  Ok(vec![v])
}

fn call_sg_node(
  (tsfn, rule): &(
    ThreadsafeFunction<PinnedNodes, ErrorStrategy::CalleeHandled>,
    RuleWithConstraint<FrontEndLanguage>,
  ),
  entry: std::result::Result<ignore::DirEntry, ignore::Error>,
) -> Ret<bool> {
  let entry = entry?;
  if !entry
    .file_type()
    .context("could not use stdin as file")?
    .is_file()
  {
    return Ok(false);
  }
  let (root, path) = get_root(entry)?;
  let mut pinned = PinnedNodeData::new(root.inner, |r| r.root().find_all(rule).collect());
  let hits: &Vec<_> = pinned.get_data();
  if hits.is_empty() {
    return Ok(false);
  }
  let pinned = PinnedNodes(pinned, path);
  tsfn.call(Ok(pinned), ThreadsafeFunctionCallMode::Blocking);
  Ok(true)
}
