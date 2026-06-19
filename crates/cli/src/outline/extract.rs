use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;

use anyhow::{Context, Result};
use ast_grep_core::Language;
use ast_grep_language::LanguageExt;
use ast_grep_outline::{
  DEFAULT_OUTLINE_RULES,
  combined_extractor::CombinedExtractors,
  extractor::{SerializableOutlineRule, parse_outline_rules},
  model::{OutlineEntry, OutlineItem, OutlineMember},
  options::OutlineExtractorOptions,
};
use ignore::WalkState;
use serde::Serialize;

use crate::lang::SgLang;
use crate::utils::{InputArgs, read_file};

use super::OutlineArg;
use super::options::{OutlineTextOptions, matches_item_matcher};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OutlineFile<'a> {
  pub(super) path: String,
  pub(super) language: String,
  pub(super) items: Vec<OutlineItem<'a>>,
}

// One command-level cache of compiled outline rules. File workers borrow this
// by language, so YAML deserialization and rule compilation never sit on the
// per-file read/parse/extract path.
pub(super) struct OutlineExtractors {
  by_lang: HashMap<SgLang, CombinedExtractors<SgLang>>,
}

impl OutlineExtractors {
  pub(super) fn try_from(
    rules: Vec<SerializableOutlineRule<SgLang>>,
    options: &OutlineExtractorOptions,
  ) -> Result<Self> {
    let mut rules_by_lang: HashMap<SgLang, Vec<SerializableOutlineRule<SgLang>>> = HashMap::new();
    for rule in rules {
      rules_by_lang
        .entry(rule.common().language)
        .or_default()
        .push(rule);
    }
    let by_lang = rules_by_lang
      .into_iter()
      .map(|(lang, rules)| {
        CombinedExtractors::try_from_rules(rules, options.clone(), &Default::default())
          .map(|extractors| (lang, extractors))
      })
      .collect::<Result<_, _>>()?;
    Ok(Self { by_lang })
  }

  fn is_empty(&self) -> bool {
    self.by_lang.is_empty()
  }

  fn languages(&self) -> impl Iterator<Item = SgLang> + '_ {
    self.by_lang.keys().copied()
  }

  fn extract<'tree>(
    &self,
    lang: SgLang,
    root: ast_grep_core::Node<'tree, ast_grep_core::tree_sitter::StrDoc<SgLang>>,
    options: &OutlineTextOptions,
  ) -> Vec<OutlineItem<'static>> {
    self
      .by_lang
      .get(&lang)
      .map(|extractors| {
        extractors
          .extract(root)
          .into_iter()
          .filter_map(|item| matches_item_matcher(&item, options).then(|| own_item(item)))
          .collect()
      })
      .unwrap_or_default()
  }
}

pub(super) fn load_outline_rules(
  include_default: bool,
  paths: &[PathBuf],
) -> Result<Vec<SerializableOutlineRule<SgLang>>> {
  let mut rules = vec![];
  if include_default {
    rules.extend(
      parse_outline_rules(DEFAULT_OUTLINE_RULES).context("Cannot parse builtin outline rules")?,
    );
  }
  for path in paths {
    let source = std::fs::read_to_string(path)
      .with_context(|| format!("Cannot read outline rules {}", path.display()))?;
    rules.extend(
      parse_outline_rules(&source)
        .with_context(|| format!("Cannot parse outline rules {}", path.display()))?,
    );
  }
  Ok(rules)
}

pub(super) fn extract_stdin(
  arg: &OutlineArg,
  extractors: &OutlineExtractors,
  options: &OutlineTextOptions,
) -> Result<OutlineFile<'static>> {
  let lang = arg.lang.expect("required by clap");
  let source = io::read_to_string(io::stdin())?;
  let grep = lang.ast_grep(source);
  let items = extractors.extract(lang, grep.root(), options);
  Ok(OutlineFile {
    path: "STDIN".to_string(),
    language: lang.to_string(),
    items,
  })
}

pub(super) fn stream_paths(
  arg: &OutlineArg,
  extractors: Arc<OutlineExtractors>,
  options: &OutlineTextOptions,
  mut emit: impl FnMut(OutlineFile<'static>) -> Result<()>,
) -> Result<()> {
  if arg.lang.is_none() && extractors.is_empty() {
    return Ok(());
  }
  let walker = outline_walk(&arg.input, arg.lang, &extractors)?;
  let (tx, rx) = mpsc::channel();
  let lang = arg.lang;
  let options = Arc::new(options.clone());
  let producer = thread::spawn(move || {
    walker.run(|| {
      let tx = tx.clone();
      let extractors = extractors.clone();
      let options = options.clone();
      Box::new(move |result| {
        let Some(path) = outline_path(result) else {
          return WalkState::Continue;
        };
        let Some(file) = extract_path_or_report(&path, lang, &extractors, &options) else {
          return WalkState::Continue;
        };
        if tx.send(file).is_err() {
          return WalkState::Quit;
        }
        WalkState::Continue
      })
    });
  });

  let mut result = Ok(());
  while let Ok(file) = rx.recv() {
    if let Err(err) = emit(file) {
      result = Err(err);
      break;
    }
  }
  drop(rx);
  producer
    .join()
    .map_err(|_| anyhow::anyhow!("outline walker thread panicked"))?;
  result
}

fn outline_walk(
  input: &InputArgs,
  lang: Option<SgLang>,
  extractors: &OutlineExtractors,
) -> Result<ignore::WalkParallel> {
  if let Some(lang) = lang {
    input.walk_lang(lang)
  } else {
    input.walk_langs(extractors.languages())
  }
}

fn outline_path(result: Result<ignore::DirEntry, ignore::Error>) -> Option<PathBuf> {
  let entry = match result {
    Ok(entry) => entry,
    Err(err) => {
      eprintln!("ERROR: {err}");
      return None;
    }
  };
  entry.file_type()?.is_file().then(|| entry.into_path())
}

fn extract_path_or_report(
  path: &Path,
  lang: Option<SgLang>,
  extractors: &OutlineExtractors,
  options: &OutlineTextOptions,
) -> Option<OutlineFile<'static>> {
  match extract_path(path, lang, extractors, options) {
    Ok(file) => file,
    Err(err) => {
      eprintln!("ERROR: {err:#}");
      None
    }
  }
}

fn extract_path(
  path: &Path,
  lang: Option<SgLang>,
  extractors: &OutlineExtractors,
  options: &OutlineTextOptions,
) -> Result<Option<OutlineFile<'static>>> {
  let Some(lang) = lang.or_else(|| SgLang::from_path(path)) else {
    return Ok(None);
  };
  let source =
    read_file(path).with_context(|| format!("Cannot extract outline from {}", path.display()))?;
  let grep = lang.ast_grep(source);
  let items = extractors.extract(lang, grep.root(), options);
  if !options.show_empty_files && items.is_empty() {
    return Ok(None);
  }
  Ok(Some(OutlineFile {
    path: path.to_string_lossy().into_owned(),
    language: lang.to_string(),
    items,
  }))
}

fn own_item(item: OutlineItem<'_>) -> OutlineItem<'static> {
  OutlineItem {
    entry: own_entry(item.entry),
    is_import: item.is_import,
    is_exported: item.is_exported,
    members: item.members.into_iter().map(own_member).collect(),
  }
}

fn own_member(member: OutlineMember<'_>) -> OutlineMember<'static> {
  OutlineMember {
    entry: own_entry(member.entry),
    is_public: member.is_public,
  }
}

fn own_entry(entry: OutlineEntry<'_>) -> OutlineEntry<'static> {
  OutlineEntry {
    role: entry.role,
    symbol_type: entry.symbol_type,
    name: Cow::Owned(entry.name.into_owned()),
    range: entry.range,
    signature: Cow::Owned(entry.signature.into_owned()),
    ast_kind: Cow::Owned(entry.ast_kind.into_owned()),
  }
}
