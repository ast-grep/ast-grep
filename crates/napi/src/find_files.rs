use ast_grep_config::RuleCore;
use ast_grep_core::pinned::{NodeData, PinnedNodeData};
use ast_grep_core::{AstGrep, NodeMatch};
use ignore::{WalkBuilder, WalkParallel, WalkState};
use napi::anyhow::{anyhow, Context, Result as Ret};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Task};
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::doc::{JsDoc, NapiConfig};
use crate::napi_lang::{build_files, LangOption, NapiLang};
use crate::sg_node::{SgNode, SgRoot};

pub struct ParseAsync {
  pub src: String,
  pub lang: NapiLang,
}

impl Task for ParseAsync {
  type Output = SgRoot;
  type JsValue = SgRoot;

  fn compute(&mut self) -> Result<Self::Output> {
    let src = std::mem::take(&mut self.src);
    let doc = JsDoc::try_new(src, self.lang)?;
    Ok(SgRoot(AstGrep::doc(doc), "anonymous".into()))
  }
  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

type Entry = std::result::Result<ignore::DirEntry, ignore::Error>;

pub struct IterateFiles<D> {
  walk: WalkParallel,
  lang_option: LangOption,
  tsfn: D,
  producer: fn(&D, Entry, &LangOption) -> Ret<bool>,
}

impl<T: 'static + Send + Sync> Task for IterateFiles<T> {
  type Output = u32;
  type JsValue = u32;

  fn compute(&mut self) -> Result<Self::Output> {
    let tsfn = &self.tsfn;
    let file_count = AtomicU32::new(0);
    let producer = self.producer;
    let walker = std::mem::replace(&mut self.walk, WalkBuilder::new(".").build_parallel());
    walker.run(|| {
      let file_count = &file_count;
      let lang_option = &self.lang_option;
      Box::new(move |entry| match producer(tsfn, entry, lang_option) {
        Ok(succeed) => {
          if succeed {
            // file is sent to JS thread, increment file count
            file_count.fetch_add(1, Ordering::AcqRel);
          }
          WalkState::Continue
        }
        Err(_) => WalkState::Skip,
      })
    });
    Ok(file_count.load(Ordering::Acquire))
  }
  fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

// See https://github.com/ast-grep/ast-grep/issues/206
// NodeJS has a 1000 file limitation on sync iteration count.
// https://github.com/nodejs/node/blob/8ba54e50496a6a5c21d93133df60a9f7cb6c46ce/src/node_api.cc#L336
const THREAD_FUNC_QUEUE_SIZE: usize = 1000;

type ParseFiles = IterateFiles<ThreadsafeFunction<SgRoot>>;

#[napi(object)]
pub struct FileOption {
  pub paths: Vec<String>,
  pub language_globs: HashMap<String, Vec<String>>,
}

#[napi]
pub fn parse_files(
  paths: Either<Vec<String>, FileOption>,
  callback: Function,
) -> Result<AsyncTask<ParseFiles>> {
  let tsfn = callback.build_threadsafe_function::<SgRoot>().callee_handled().build()?;
  let (paths, globs) = match paths {
    Either::A(v) => (v, HashMap::new()),
    Either::B(FileOption {
      paths,
      language_globs,
    }) => (paths, NapiLang::lang_globs(language_globs)),
  };
  let walk = build_files(paths, &globs)?;
  Ok(AsyncTask::new(ParseFiles {
    walk,
    tsfn,
    lang_option: LangOption::infer(&globs),
    producer: call_sg_root,
  }))
}

// returns if the entry is a file and sent to JavaScript queue
fn call_sg_root(
  tsfn: &ThreadsafeFunction<SgRoot>,
  entry: std::result::Result<ignore::DirEntry, ignore::Error>,
  lang_option: &LangOption,
) -> Ret<bool> {
  let entry = entry?;
  if !entry
    .file_type()
    .context("could not use stdin as file")?
    .is_file()
  {
    return Ok(false);
  }
  let (root, path) = get_root(entry, lang_option)?;
  let sg = SgRoot(root, path);
  tsfn.call(Ok(sg), ThreadsafeFunctionCallMode::Blocking);
  Ok(true)
}

fn get_root(entry: ignore::DirEntry, lang_option: &LangOption) -> Ret<(AstGrep<JsDoc>, String)> {
  let path = entry.into_path();
  let file_content = std::fs::read_to_string(&path)?;
  let lang = lang_option
    .get_lang(&path)
    .context(anyhow!("file not recognized"))?;
  let doc = JsDoc::try_new(file_content, lang)?;
  Ok((AstGrep::doc(doc), path.to_string_lossy().into()))
}

pub type FindInFiles = IterateFiles<(
  ThreadsafeFunction<()>,
  RuleCore,
)>;

pub struct PinnedNodes(
  PinnedNodeData<JsDoc, Vec<NodeMatch<'static, JsDoc>>>,
  String,
);
unsafe impl Send for PinnedNodes {}
unsafe impl Sync for PinnedNodes {}

#[napi(object)]
pub struct FindConfig {
  /// specify the file paths to recursively find files
  pub paths: Vec<String>,
  /// a Rule object to find what nodes will match
  pub matcher: NapiConfig,
  /// An list of pattern globs to treat of certain files in the specified language.
  /// eg. ['*.vue', '*.svelte'] for html.findFiles, or ['*.ts'] for tsx.findFiles.
  /// It is slightly different from https://ast-grep.github.io/reference/sgconfig.html#languageglobs
  pub language_globs: Option<Vec<String>>,
}

pub fn find_in_files_impl(
  lang: NapiLang,
  config: FindConfig,
  callback: Function,
) -> Result<AsyncTask<FindInFiles>> {
  let tsfn = callback.build_threadsafe_function::<()>().callee_handled().build()?;
  let FindConfig {
    paths,
    matcher,
    language_globs,
  } = config;
  let rule = matcher.parse_with(lang)?;
  let walk = lang.find_files(paths, language_globs)?;
  Ok(AsyncTask::new(FindInFiles {
    walk,
    tsfn: (tsfn, rule),
    lang_option: LangOption::Specified(lang),
    producer: call_sg_node,
  }))
}

// TODO: optimize
fn from_pinned_data(pinned: PinnedNodes, env: napi::Env) -> Result<Vec<Vec<SgNode>>> {
  let (root, nodes) = pinned.0.into_raw();
  let sg_root = SgRoot(root, pinned.1);
  let reference = SgRoot::into_reference(sg_root, env)?;
  let mut v = vec![];
  for mut node in nodes {
    let root_ref = reference.clone(env)?;
    let sg_node = SgNode {
      inner: root_ref.share_with(env, |root| {
        let r = &root.0;
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
    ThreadsafeFunction<()>,
    RuleCore,
  ),
  entry: std::result::Result<ignore::DirEntry, ignore::Error>,
  lang_option: &LangOption,
) -> Ret<bool> {
  let entry = entry?;
  if !entry
    .file_type()
    .context("could not use stdin as file")?
    .is_file()
  {
    return Ok(false);
  }
  let (root, path) = get_root(entry, lang_option)?;
  let mut pinned = PinnedNodeData::new(root, |r| r.root().find_all(rule).collect());
  let hits: &Vec<_> = pinned.get_data();
  if hits.is_empty() {
    return Ok(false);
  }
  let pinned = PinnedNodes(pinned, path);
  tsfn.call(Ok(()), ThreadsafeFunctionCallMode::Blocking);
  Ok(true)
}
