use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use ast_grep_core::Language;
use ast_grep_language::LanguageExt;
use ast_grep_outline::{
  DEFAULT_OUTLINE_RULES,
  combined_extractor::CombinedExtractors,
  extractor::{SerializableOutlineRule, parse_outline_rules},
  model::{OutlineEntry, OutlineItem, OutlineMember, SymbolType},
};
use clap::{Args, ValueEnum};
use ignore::WalkState;
use regex::Regex;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Display;
use std::sync::mpsc;

use crate::lang::SgLang;
use crate::print::{ColorArg, JsonStyle};
use crate::utils::{InputArgs, read_file};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutlineItems {
  /// Use `structure` for file or stdin input, `exports` when any directory is given.
  Auto,
  /// Top-level items defined locally in the file, excluding imports.
  Structure,
  /// Top-level items exported from the file or module.
  Exports,
  /// Top-level items imported from other files or modules.
  Imports,
  /// All top-level items, including imports and exports.
  All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutlineView {
  /// Use `digest` for file or stdin input, `names` when any directory is given.
  Auto,
  /// One grouped name line per symbol type for each file.
  Names,
  /// One source/signature line per top-level item.
  Signatures,
  /// Signatures plus compact member name digests.
  Digest,
  /// Signatures plus one source/signature line per direct member.
  Expanded,
}

#[derive(Args)]
pub struct OutlineArg {
  /// Specify the input language.
  ///
  /// For path input, ast-grep parses only files of this language. For stdin,
  /// this flag is required because there is no file path to infer the language from.
  #[clap(short, long, required_if_eq("stdin", "true"))]
  lang: Option<SgLang>,

  /// Output outline entries in structured JSON.
  ///
  /// If this flag is set, ast-grep will output outline entries in JSON format.
  /// You can pass optional value to this flag by using `--json=<STYLE>` syntax
  /// to further control how JSON object is formatted and printed. ast-grep will
  /// `pretty`-print JSON if no value is passed.
  /// Note, the json flag must use `=` to specify its value.
  #[clap(
      long,
      value_name="STYLE",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pretty"
  )]
  json: Option<JsonStyle>,

  /// Controls output color.
  ///
  /// This flag controls when to use colors. The default setting is 'auto', which
  /// means ast-grep will try to guess when to use colors. If ast-grep is
  /// printing to a terminal, then it will use colors, but if it is redirected to a
  /// file or a pipe, then it will suppress color output. ast-grep will also suppress
  /// color output in some other circumstances. For example, no color will be used
  /// if the TERM environment variable is not set or set to 'dumb'.
  #[clap(long, default_value = "auto", value_name = "WHEN")]
  color: ColorArg,

  /// Select which top-level items to include.
  ///
  /// This option controls top-level structure such as classes, structs, interfaces,
  /// functions, and modules. It does not filter members.
  /// By default, ast-grep picks the items automatically based on the input path.
  #[clap(long, default_value = "auto", value_name = "ITEMS")]
  items: OutlineItems,

  /// Keep only top-level items with these comma-separated symbol types.
  ///
  /// For example, `--type class,enum` keeps both classes and enums.
  #[clap(long = "type", value_name = "TYPE[,TYPE...]")]
  symbol_type: Option<String>,

  /// Keep only top-level items matching this regex.
  ///
  /// The regex is matched against item names, signatures, first source lines,
  /// and import/export item signatures. It never matches members.
  #[clap(long = "match", value_name = "REGEX")]
  match_item: Option<String>,

  /// Display only public members in member views.
  ///
  /// By default, member views display all extracted members; the digest view
  /// lists public members before non-public members.
  #[clap(long)]
  pub_members: bool,

  /// Select the text presentation.
  ///
  /// Views contain increasingly more information, from grouped names to expanded
  /// member signatures.
  /// By default, ast-grep picks the view automatically based on the input path.
  #[clap(long, default_value = "auto", value_name = "VIEW")]
  view: OutlineView,

  /// Load additional outline extractor definitions.
  #[clap(long, action = clap::ArgAction::Append, value_name = "FILE")]
  outline_rules: Vec<PathBuf>,

  /// Do not load bundled outline extractor definitions.
  #[clap(long)]
  no_default_outline_rules: bool,

  /// Input related options.
  #[clap(flatten)]
  input: InputArgs,
}

pub fn run_outline(arg: OutlineArg) -> anyhow::Result<ExitCode> {
  let rules = load_outline_rules(!arg.no_default_outline_rules, &arg.outline_rules)?;
  let extractors = Arc::new(OutlineExtractors::try_from(rules)?);
  let options = OutlineTextOptions::try_from(&arg)?;
  let stdout = io::stdout();
  let mut emitter = OutlineEmitter::new(io::BufWriter::new(stdout.lock()), arg.json, &options);
  if arg.input.stdin {
    emitter.emit(extract_stdin(&arg, &extractors, &options)?)?;
  } else {
    stream_paths(&arg, extractors, &options, &mut emitter)?;
  }
  emitter.finish()?;
  Ok(ExitCode::SUCCESS)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineFile<'a> {
  path: String,
  language: String,
  items: Vec<OutlineItem<'a>>,
}

// One command-level cache of compiled outline rules. File workers borrow this
// by language, so YAML deserialization and rule compilation never sit on the
// per-file read/parse/extract path.
struct OutlineExtractors {
  by_lang: HashMap<SgLang, CombinedExtractors<SgLang>>,
}

impl OutlineExtractors {
  fn try_from(rules: Vec<SerializableOutlineRule<SgLang>>) -> Result<Self> {
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
        CombinedExtractors::try_from(rules, &Default::default())
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
          .filter_map(|mut item| {
            if options.pub_members {
              item.members.retain(|member| member.is_public);
            }
            matches_item_filters(&item, options).then(|| own_item(item))
          })
          .collect()
      })
      .unwrap_or_default()
  }
}

fn load_outline_rules(
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

fn extract_stdin(
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

fn stream_paths(
  arg: &OutlineArg,
  extractors: Arc<OutlineExtractors>,
  options: &OutlineTextOptions,
  emitter: &mut OutlineEmitter<impl Write>,
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
    if let Err(err) = emitter.emit(file) {
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

#[derive(Clone)]
struct OutlineTextOptions {
  items: OutlineItems,
  view: OutlineView,
  symbol_types: Option<Vec<SymbolType>>,
  item_matcher: Option<Regex>,
  pub_members: bool,
  use_color: bool,
  show_empty_files: bool,
}

impl OutlineTextOptions {
  fn try_from(arg: &OutlineArg) -> Result<Self> {
    let has_directory_input = !arg.input.stdin && arg.input.paths.iter().any(|path| path.is_dir());
    Ok(Self {
      items: resolve_items(arg.items, has_directory_input),
      view: resolve_view(arg.view, has_directory_input),
      symbol_types: arg
        .symbol_type
        .as_deref()
        .map(parse_symbol_types)
        .transpose()?,
      item_matcher: arg
        .match_item
        .as_deref()
        .map(Regex::new)
        .transpose()
        .context("Cannot parse outline item matcher")?,
      pub_members: arg.pub_members,
      use_color: arg.color.should_use_color(),
      show_empty_files: arg.input.stdin || !has_directory_input,
    })
  }
}

fn resolve_items(items: OutlineItems, has_directory_input: bool) -> OutlineItems {
  match (items, has_directory_input) {
    (OutlineItems::Auto, true) => OutlineItems::Exports,
    (OutlineItems::Auto, false) => OutlineItems::Structure,
    _ => items,
  }
}

fn resolve_view(view: OutlineView, has_directory_input: bool) -> OutlineView {
  match (view, has_directory_input) {
    (OutlineView::Auto, true) => OutlineView::Names,
    (OutlineView::Auto, false) => OutlineView::Digest,
    _ => view,
  }
}

fn parse_symbol_types(source: &str) -> Result<Vec<SymbolType>> {
  source
    .split(',')
    .map(|raw| {
      let raw = raw.trim();
      parse_symbol_type(raw).with_context(|| format!("Unknown outline symbol type `{raw}`"))
    })
    .collect()
}

fn parse_symbol_type(raw: &str) -> Option<SymbolType> {
  let normalized = raw
    .chars()
    .filter(|ch| *ch != '-' && *ch != '_')
    .flat_map(char::to_lowercase)
    .collect::<String>();
  Some(match normalized.as_str() {
    "file" => SymbolType::File,
    "module" => SymbolType::Module,
    "namespace" => SymbolType::Namespace,
    "package" => SymbolType::Package,
    "class" => SymbolType::Class,
    "method" => SymbolType::Method,
    "property" => SymbolType::Property,
    "field" => SymbolType::Field,
    "constructor" => SymbolType::Constructor,
    "enum" => SymbolType::Enum,
    "interface" => SymbolType::Interface,
    "function" => SymbolType::Function,
    "variable" => SymbolType::Variable,
    "constant" => SymbolType::Constant,
    "string" => SymbolType::String,
    "number" => SymbolType::Number,
    "boolean" => SymbolType::Boolean,
    "array" => SymbolType::Array,
    "object" => SymbolType::Object,
    "key" => SymbolType::Key,
    "null" => SymbolType::Null,
    "enummember" => SymbolType::EnumMember,
    "struct" => SymbolType::Struct,
    "event" => SymbolType::Event,
    "operator" => SymbolType::Operator,
    "typeparameter" => SymbolType::TypeParameter,
    _ => return None,
  })
}

fn matches_item_filters(item: &OutlineItem, options: &OutlineTextOptions) -> bool {
  matches_items_mode(item, options.items)
    && matches_symbol_type(item, options.symbol_types.as_deref())
    && matches_item_regex(item, options.item_matcher.as_ref())
}

fn matches_items_mode(item: &OutlineItem, items: OutlineItems) -> bool {
  match items {
    OutlineItems::Auto => unreachable!("outline item mode should be resolved"),
    OutlineItems::Structure => !item.is_import,
    OutlineItems::Exports => item.is_exported,
    OutlineItems::Imports => item.is_import,
    OutlineItems::All => true,
  }
}

fn matches_symbol_type(item: &OutlineItem, symbol_types: Option<&[SymbolType]>) -> bool {
  symbol_types.is_none_or(|types| types.contains(&item.entry.symbol_type))
}

fn matches_item_regex(item: &OutlineItem, matcher: Option<&Regex>) -> bool {
  matcher.is_none_or(|matcher| {
    matcher.is_match(&item.entry.name) || matcher.is_match(&item.entry.signature)
  })
}

struct OutlineEmitter<'a, W> {
  out: W,
  json: Option<JsonStyle>,
  options: &'a OutlineTextOptions,
  text_style: OutlineTextStyle,
  is_first: bool,
  emitted_any: bool,
}

impl<'a, W: Write> OutlineEmitter<'a, W> {
  fn new(out: W, json: Option<JsonStyle>, options: &'a OutlineTextOptions) -> Self {
    Self {
      out,
      json,
      options,
      text_style: OutlineTextStyle::new(options.use_color, options.items),
      is_first: true,
      emitted_any: false,
    }
  }

  fn emit(&mut self, file: OutlineFile<'static>) -> Result<()> {
    match self.json {
      Some(JsonStyle::Pretty) => self.emit_pretty_json(&file)?,
      Some(JsonStyle::Compact) => self.emit_compact_json(&file)?,
      Some(JsonStyle::Stream) => {
        serde_json::to_writer(&mut self.out, &file)?;
        writeln!(self.out)?;
      }
      None => print_text_file_to(
        &mut self.out,
        &file,
        self.options,
        &self.text_style,
        self.is_first,
      )?,
    }
    self.is_first = false;
    self.emitted_any = true;
    self.out.flush()?;
    Ok(())
  }

  fn finish(&mut self) -> Result<()> {
    match self.json {
      Some(JsonStyle::Pretty) => {
        if self.emitted_any {
          writeln!(self.out, "]")?;
        } else {
          writeln!(self.out, "[]")?;
        }
      }
      Some(JsonStyle::Compact) => {
        if self.emitted_any {
          writeln!(self.out, "]")?;
        } else {
          writeln!(self.out, "[]")?;
        }
      }
      Some(JsonStyle::Stream) => {}
      None if !self.emitted_any => {
        writeln!(self.out, "nothing found")?;
      }
      None => {}
    }
    self.out.flush()?;
    Ok(())
  }

  fn emit_pretty_json(&mut self, file: &OutlineFile) -> Result<()> {
    if self.is_first {
      writeln!(self.out, "[")?;
    } else {
      writeln!(self.out, ",")?;
    }
    let object = serde_json::to_string_pretty(file)?;
    for line in object.lines() {
      writeln!(self.out, "  {line}")?;
    }
    Ok(())
  }

  fn emit_compact_json(&mut self, file: &OutlineFile) -> Result<()> {
    if self.is_first {
      write!(self.out, "[")?;
    } else {
      write!(self.out, ",")?;
    }
    serde_json::to_writer(&mut self.out, file)?;
    Ok(())
  }
}

#[cfg(test)]
fn print_text_to(
  mut out: &mut impl Write,
  files: &[OutlineFile],
  options: &OutlineTextOptions,
) -> Result<()> {
  let style = OutlineTextStyle::new(options.use_color, options.items);
  if files.is_empty() {
    writeln!(out, "nothing found")?;
    return Ok(());
  }
  for (idx, file) in files.iter().enumerate() {
    print_text_file_to(&mut out, file, options, &style, idx == 0)?;
  }
  Ok(())
}

fn print_text_file_to(
  mut out: &mut impl Write,
  file: &OutlineFile,
  options: &OutlineTextOptions,
  style: &OutlineTextStyle,
  is_first: bool,
) -> Result<()> {
  if !is_first {
    writeln!(out)?;
  }
  writeln!(out, "{}", style.file(&file.path))?;
  if file.items.is_empty() {
    writeln!(out, "nothing found")?;
  } else {
    let line_number_width = line_number_width(file);
    match options.view {
      OutlineView::Auto => unreachable!("outline view should be resolved"),
      OutlineView::Names => print_names(&mut out, file, style)?,
      OutlineView::Signatures => print_signatures(&mut out, file, style, line_number_width)?,
      OutlineView::Digest => print_digest(&mut out, file, style, line_number_width)?,
      OutlineView::Expanded => print_expanded(&mut out, file, style, line_number_width)?,
    }
  }
  Ok(())
}

fn print_names(out: &mut impl Write, file: &OutlineFile, style: &OutlineTextStyle) -> Result<()> {
  for (symbol_type, names) in grouped_item_names(&file.items) {
    writeln!(
      out,
      "{}: {}",
      style.grouped_label(symbol_type, symbol_type_name(symbol_type)),
      names
        .iter()
        .map(|name| style.grouped_item_name(name))
        .collect::<Vec<_>>()
        .join(", ")
    )?;
  }
  Ok(())
}

fn print_signatures(
  out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> Result<()> {
  for item in &file.items {
    writeln!(out, "{}", item_line(item, style, true, line_number_width))?;
  }
  Ok(())
}

fn print_digest(
  out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> Result<()> {
  let member_indent = grouped_member_indent(line_number_width);
  for item in &file.items {
    writeln!(out, "{}", item_line(item, style, true, line_number_width))?;
    for (symbol_type, names) in grouped_member_names(&item.members) {
      writeln!(
        out,
        "{}{}: {}",
        member_indent,
        style.grouped_label(symbol_type, plural_symbol_type_name(symbol_type)),
        names
          .iter()
          .map(|name| style.grouped_member_name(name))
          .collect::<Vec<_>>()
          .join(", ")
      )?;
    }
  }
  Ok(())
}

fn grouped_member_indent(line_number_width: usize) -> String {
  " ".repeat(line_number_width + 4)
}

fn print_expanded(
  out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> Result<()> {
  for item in &file.items {
    writeln!(out, "{}", item_line(item, style, true, line_number_width))?;
    for member in &item.members {
      writeln!(out, "{}", member_line(member, style, line_number_width))?;
    }
  }
  Ok(())
}

fn line_number_width(file: &OutlineFile) -> usize {
  file
    .items
    .iter()
    .flat_map(|item| {
      std::iter::once(item.entry.range.start.line + 1).chain(
        item
          .members
          .iter()
          .map(|member| member.entry.range.start.line + 1),
      )
    })
    .max()
    .unwrap_or(1)
    .to_string()
    .len()
}

#[derive(Clone)]
struct StyledName {
  text: String,
  is_import: bool,
  is_exported: bool,
  is_public: bool,
}

fn grouped_item_names(items: &[OutlineItem]) -> Vec<(SymbolType, Vec<StyledName>)> {
  let mut groups = empty_symbol_groups();
  for item in items {
    push_grouped_name(
      &mut groups,
      item.entry.symbol_type,
      StyledName {
        text: item.entry.name.to_string(),
        is_import: item.is_import,
        is_exported: item.is_exported,
        is_public: true,
      },
    );
  }
  groups.retain(|(_, names)| !names.is_empty());
  groups
}

fn grouped_member_names(members: &[OutlineMember]) -> Vec<(SymbolType, Vec<StyledName>)> {
  let mut groups = empty_symbol_groups();
  for member in members.iter().filter(|member| member.is_public) {
    push_grouped_name(
      &mut groups,
      member.entry.symbol_type,
      StyledName {
        text: member.entry.name.to_string(),
        is_import: false,
        is_exported: false,
        is_public: member.is_public,
      },
    );
  }
  for member in members.iter().filter(|member| !member.is_public) {
    push_grouped_name(
      &mut groups,
      member.entry.symbol_type,
      StyledName {
        text: member.entry.name.to_string(),
        is_import: false,
        is_exported: false,
        is_public: member.is_public,
      },
    );
  }
  groups.retain(|(_, names)| !names.is_empty());
  groups
}

fn empty_symbol_groups() -> Vec<(SymbolType, Vec<StyledName>)> {
  SYMBOL_TYPE_ORDER
    .iter()
    .map(|&symbol_type| (symbol_type, vec![]))
    .collect()
}

fn push_grouped_name(
  groups: &mut Vec<(SymbolType, Vec<StyledName>)>,
  symbol_type: SymbolType,
  name: StyledName,
) {
  if let Some((_, names)) = groups.iter_mut().find(|(ty, _)| *ty == symbol_type) {
    names.push(name);
  } else {
    groups.push((symbol_type, vec![name]));
  }
}

fn item_line(
  item: &OutlineItem,
  style: &OutlineTextStyle,
  emphasize_exported: bool,
  line_number_width: usize,
) -> String {
  format!(
    "{}: {}",
    style.line_number(format_args!(
      "{:>line_number_width$}",
      item.entry.range.start.line + 1
    )),
    style.entry_signature(
      &item.entry,
      item.entry.symbol_type,
      item.is_import,
      emphasize_exported && item.is_exported
    )
  )
}

fn member_line(
  member: &OutlineMember,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> String {
  format!(
    "{}:   {}",
    style.line_number(format_args!(
      "{:>line_number_width$}",
      member.entry.range.start.line + 1
    )),
    style.member_signature(member, member.entry.symbol_type)
  )
}

fn signature_or_name<'entry, 'text>(entry: &'entry OutlineEntry<'text>) -> &'entry str {
  if entry.signature.is_empty() {
    &entry.name
  } else {
    &entry.signature
  }
}

fn symbol_type_name(symbol_type: SymbolType) -> &'static str {
  match symbol_type {
    SymbolType::File => "file",
    SymbolType::Module => "module",
    SymbolType::Namespace => "namespace",
    SymbolType::Package => "package",
    SymbolType::Class => "class",
    SymbolType::Method => "method",
    SymbolType::Property => "property",
    SymbolType::Field => "field",
    SymbolType::Constructor => "constructor",
    SymbolType::Enum => "enum",
    SymbolType::Interface => "interface",
    SymbolType::Function => "function",
    SymbolType::Variable => "variable",
    SymbolType::Constant => "constant",
    SymbolType::String => "string",
    SymbolType::Number => "number",
    SymbolType::Boolean => "boolean",
    SymbolType::Array => "array",
    SymbolType::Object => "object",
    SymbolType::Key => "key",
    SymbolType::Null => "null",
    SymbolType::EnumMember => "enumMember",
    SymbolType::Struct => "struct",
    SymbolType::Event => "event",
    SymbolType::Operator => "operator",
    SymbolType::TypeParameter => "typeParameter",
  }
}

fn plural_symbol_type_name(symbol_type: SymbolType) -> &'static str {
  match symbol_type {
    SymbolType::Class => "classes",
    SymbolType::Property => "properties",
    SymbolType::Enum => "enums",
    SymbolType::Struct => "structs",
    SymbolType::TypeParameter => "typeParameters",
    _ => match symbol_type_name(symbol_type) {
      "boolean" => "booleans",
      "enumMember" => "enumMembers",
      "namespace" => "namespaces",
      "constructor" => "constructors",
      "interface" => "interfaces",
      "function" => "functions",
      "variable" => "variables",
      "constant" => "constants",
      "operator" => "operators",
      "package" => "packages",
      "module" => "modules",
      "method" => "methods",
      "field" => "fields",
      "event" => "events",
      "array" => "arrays",
      "file" => "files",
      "string" => "strings",
      "number" => "numbers",
      "object" => "objects",
      "key" => "keys",
      "null" => "nulls",
      _ => symbol_type_name(symbol_type),
    },
  }
}

const SYMBOL_TYPE_ORDER: &[SymbolType] = &[
  SymbolType::File,
  SymbolType::Module,
  SymbolType::Namespace,
  SymbolType::Package,
  SymbolType::Class,
  SymbolType::Struct,
  SymbolType::Enum,
  SymbolType::Interface,
  SymbolType::Function,
  SymbolType::Method,
  SymbolType::Constructor,
  SymbolType::Property,
  SymbolType::Field,
  SymbolType::EnumMember,
  SymbolType::Variable,
  SymbolType::Constant,
  SymbolType::String,
  SymbolType::Number,
  SymbolType::Boolean,
  SymbolType::Array,
  SymbolType::Object,
  SymbolType::Key,
  SymbolType::Null,
  SymbolType::Event,
  SymbolType::Operator,
  SymbolType::TypeParameter,
];

struct OutlineTextStyle {
  use_color: bool,
  emphasize_imports: bool,
  emphasize_exports: bool,
}

impl OutlineTextStyle {
  fn new(use_color: bool, items: OutlineItems) -> Self {
    Self {
      use_color,
      emphasize_imports: items != OutlineItems::Imports,
      emphasize_exports: items != OutlineItems::Exports,
    }
  }

  fn file(&self, text: impl Display) -> String {
    self.paint(ansi_term::Style::new().underline(), text)
  }

  fn line_number(&self, text: impl Display) -> String {
    self.paint(ansi_term::Style::new().dimmed(), text)
  }

  fn grouped_label(&self, symbol_type: SymbolType, text: impl Display) -> String {
    let text = text.to_string();
    if !self.use_color {
      return text;
    }
    symbol_type_style(symbol_type).paint(text).to_string()
  }

  fn entry_signature(
    &self,
    entry: &OutlineEntry,
    symbol_type: SymbolType,
    is_import: bool,
    emphasize_name: bool,
  ) -> String {
    let name_style = item_name_style(
      symbol_type,
      self.emphasize_imports && is_import,
      self.emphasize_exports && emphasize_name,
    );
    self.signature(entry, name_style, None)
  }

  fn member_signature(&self, member: &OutlineMember, symbol_type: SymbolType) -> String {
    let surrounding_style = if member.is_public {
      None
    } else {
      Some(ansi_term::Style::new().dimmed())
    };
    let name_style = if member.is_public {
      symbol_type_style(symbol_type)
    } else {
      symbol_type_style(symbol_type).dimmed()
    };
    self.signature(&member.entry, name_style, surrounding_style)
  }

  fn grouped_item_name(&self, name: &StyledName) -> String {
    let mut style = ansi_term::Style::new();
    if self.emphasize_imports && name.is_import {
      style = style.italic();
    }
    if self.emphasize_exports && name.is_exported {
      style = style.bold();
    }
    self.paint(style, &name.text)
  }

  fn grouped_member_name(&self, name: &StyledName) -> String {
    let style = if name.is_public {
      ansi_term::Style::new()
    } else {
      ansi_term::Style::new().dimmed()
    };
    self.paint(style, &name.text)
  }

  fn paint(&self, style: ansi_term::Style, text: impl Display) -> String {
    if self.use_color {
      style.paint(text.to_string()).to_string()
    } else {
      text.to_string()
    }
  }

  fn signature(
    &self,
    entry: &OutlineEntry,
    name_style: ansi_term::Style,
    surrounding_style: Option<ansi_term::Style>,
  ) -> String {
    let signature = signature_or_name(entry);
    if !self.use_color {
      return signature.to_string();
    }
    let Some(start) = signature.find(entry.name.as_ref()) else {
      return surrounding_style.map_or_else(
        || signature.to_string(),
        |style| self.paint(style, signature),
      );
    };
    let end = start + entry.name.len();
    let before = surrounding_style.map_or_else(
      || signature[..start].to_string(),
      |style| self.paint(style, &signature[..start]),
    );
    let name = name_style.paint(&signature[start..end]).to_string();
    let after = surrounding_style.map_or_else(
      || signature[end..].to_string(),
      |style| self.paint(style, &signature[end..]),
    );
    format!("{before}{name}{after}")
  }
}

fn symbol_type_style(symbol_type: SymbolType) -> ansi_term::Style {
  use ansi_term::Color;
  let color = match symbol_type {
    SymbolType::File | SymbolType::Module | SymbolType::Namespace | SymbolType::Package => {
      Color::Cyan
    }
    SymbolType::Class | SymbolType::Struct | SymbolType::Object => Color::Blue,
    SymbolType::Enum | SymbolType::EnumMember => Color::Purple,
    SymbolType::Interface | SymbolType::TypeParameter => Color::Fixed(39),
    SymbolType::Function | SymbolType::Method | SymbolType::Constructor => Color::Green,
    SymbolType::Property | SymbolType::Field | SymbolType::Key => Color::Yellow,
    SymbolType::Variable | SymbolType::Constant => Color::Fixed(214),
    SymbolType::String
    | SymbolType::Number
    | SymbolType::Boolean
    | SymbolType::Array
    | SymbolType::Null => Color::Fixed(208),
    SymbolType::Event | SymbolType::Operator => Color::Red,
  };
  color.normal()
}

fn item_name_style(
  symbol_type: SymbolType,
  is_import: bool,
  is_exported: bool,
) -> ansi_term::Style {
  let mut style = symbol_type_style(symbol_type);
  if is_import {
    style = style.italic();
  }
  if is_exported {
    style = style.bold();
  }
  style
}

#[cfg(test)]
mod tests {
  use super::*;
  use ast_grep_outline::model::{EntryRole, SourcePosition, SourceRange};

  fn options(view: OutlineView) -> OutlineTextOptions {
    OutlineTextOptions {
      items: OutlineItems::All,
      view,
      symbol_types: None,
      item_matcher: None,
      pub_members: false,
      use_color: false,
      show_empty_files: true,
    }
  }

  fn range(line: usize) -> SourceRange {
    SourceRange {
      byte_offset: 0..0,
      start: SourcePosition { line, column: 0 },
      end: SourcePosition { line, column: 0 },
    }
  }

  fn entry(
    role: EntryRole,
    symbol_type: SymbolType,
    name: &'static str,
    signature: &'static str,
    line: usize,
  ) -> OutlineEntry<'static> {
    OutlineEntry {
      role,
      symbol_type,
      name: Cow::Borrowed(name),
      range: range(line),
      signature: Cow::Borrowed(signature),
      ast_kind: Cow::Borrowed("test_node"),
    }
  }

  fn member(
    symbol_type: SymbolType,
    name: &'static str,
    signature: &'static str,
    line: usize,
    is_public: bool,
  ) -> OutlineMember<'static> {
    OutlineMember {
      entry: entry(EntryRole::Member, symbol_type, name, signature, line),
      is_public,
    }
  }

  fn outline_file() -> OutlineFile<'static> {
    OutlineFile {
      path: "src/parser.ts".to_string(),
      language: "TypeScript".to_string(),
      items: vec![OutlineItem {
        entry: entry(
          EntryRole::Item,
          SymbolType::Class,
          "Parser",
          "export class Parser",
          39,
        ),
        is_import: false,
        is_exported: true,
        members: vec![
          member(SymbolType::Method, "parse", "parse(...)", 43, true),
          member(SymbolType::Method, "recover", "recover(...)", 72, false),
        ],
      }],
    }
  }

  #[test]
  fn renders_digest_like_design_doc() {
    let mut output = vec![];
    print_text_to(
      &mut output,
      &[outline_file()],
      &options(OutlineView::Digest),
    )
    .expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert_eq!(
      output,
      "src/parser.ts\n40: export class Parser\n      methods: parse, recover\n"
    );
  }

  #[test]
  fn renders_expanded_members_like_design_doc() {
    let mut output = vec![];
    print_text_to(
      &mut output,
      &[outline_file()],
      &options(OutlineView::Expanded),
    )
    .expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert_eq!(
      output,
      "src/parser.ts\n40: export class Parser\n44:   parse(...)\n73:   recover(...)\n"
    );
  }

  #[test]
  fn aligns_line_numbers_to_file_width() {
    let mut file = outline_file();
    file.items.push(OutlineItem {
      entry: entry(EntryRole::Item, SymbolType::Function, "late", "late()", 99),
      is_import: false,
      is_exported: false,
      members: vec![],
    });
    let mut output = vec![];
    print_text_to(&mut output, &[file], &options(OutlineView::Expanded))
      .expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert_eq!(
      output,
      "src/parser.ts\n 40: export class Parser\n 44:   parse(...)\n 73:   recover(...)\n100: late()\n"
    );

    let mut file = outline_file();
    file.items.push(OutlineItem {
      entry: entry(EntryRole::Item, SymbolType::Function, "late", "late()", 99),
      is_import: false,
      is_exported: false,
      members: vec![],
    });
    let mut output = vec![];
    print_text_to(&mut output, &[file], &options(OutlineView::Digest)).expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert!(output.contains("\n       methods: parse, recover\n"));
  }

  #[test]
  fn renders_empty_direct_file_block() {
    let mut output = vec![];
    let file = OutlineFile {
      path: "src/empty.ts".to_string(),
      language: "TypeScript".to_string(),
      items: vec![],
    };
    print_text_to(&mut output, &[file], &options(OutlineView::Digest)).expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert_eq!(output, "src/empty.ts\nnothing found\n");
  }

  #[test]
  fn separates_file_blocks_with_blank_line() {
    let mut output = vec![];
    let first = outline_file();
    let mut second = outline_file();
    second.path = "src/checker.ts".to_string();
    print_text_to(
      &mut output,
      &[first, second],
      &options(OutlineView::Signatures),
    )
    .expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert!(output.contains("src/parser.ts\n40: export class Parser\n\nsrc/checker.ts\n"));
  }

  #[test]
  fn emitter_streams_text_file_blocks() {
    let options = options(OutlineView::Signatures);
    let mut output = vec![];
    {
      let mut emitter = OutlineEmitter::new(&mut output, None, &options);
      emitter.emit(outline_file()).expect("file should emit");
      let mut second = outline_file();
      second.path = "src/checker.ts".to_string();
      emitter.emit(second).expect("file should emit");
      emitter.finish().expect("output should finish");
    }
    let output = String::from_utf8(output).expect("output should be utf8");

    assert!(output.contains("src/parser.ts\n40: export class Parser\n\nsrc/checker.ts\n"));
  }

  #[test]
  fn emitter_streams_json_lines_per_file() {
    let options = options(OutlineView::Signatures);
    let mut output = vec![];
    {
      let mut emitter = OutlineEmitter::new(&mut output, Some(JsonStyle::Stream), &options);
      emitter.emit(outline_file()).expect("file should emit");
      let mut second = outline_file();
      second.path = "src/checker.ts".to_string();
      emitter.emit(second).expect("file should emit");
      emitter.finish().expect("output should finish");
    }
    let output = String::from_utf8(output).expect("output should be utf8");
    let lines = output.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);
    for line in lines {
      serde_json::from_str::<serde_json::Value>(line).expect("line should be json");
    }
  }

  #[test]
  fn emitter_streams_valid_compact_json_array() {
    let options = options(OutlineView::Signatures);
    let mut output = vec![];
    {
      let mut emitter = OutlineEmitter::new(&mut output, Some(JsonStyle::Compact), &options);
      emitter.emit(outline_file()).expect("file should emit");
      let mut second = outline_file();
      second.path = "src/checker.ts".to_string();
      emitter.emit(second).expect("file should emit");
      emitter.finish().expect("output should finish");
    }
    let output: serde_json::Value =
      serde_json::from_slice(&output).expect("output should be a json array");

    assert_eq!(output.as_array().expect("json should be array").len(), 2);
  }

  #[test]
  fn signature_view_styles_exported_items() {
    let mut options = options(OutlineView::Signatures);
    options.use_color = true;
    let mut output = vec![];
    print_text_to(&mut output, &[outline_file()], &options).expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert!(output.contains("export class \u{1b}[1;34mParser\u{1b}[0m"));
  }

  #[test]
  fn colors_symbol_types_differently() {
    let style = OutlineTextStyle::new(true, OutlineItems::All);
    let class = style.grouped_label(SymbolType::Class, "class");
    let function = style.grouped_label(SymbolType::Function, "function");
    let label = style.grouped_label(SymbolType::Function, "functions");

    assert_ne!(class, function);
    assert!(class.contains("\u{1b}["));
    assert!(function.contains("\u{1b}["));
    assert!(label.contains("\u{1b}["));
    assert!(!label.contains("\u{1b}[7;"));
    assert!(label.contains("functions"));
  }

  #[test]
  fn styles_outline_flags_with_ansi() {
    let style = OutlineTextStyle::new(true, OutlineItems::All);
    let file = style.file("src/parser.ts");
    let import = style.entry_signature(
      &entry(
        EntryRole::Item,
        SymbolType::Module,
        "std::fmt",
        "use std::fmt;",
        0,
      ),
      SymbolType::Module,
      true,
      false,
    );
    let exported = style.entry_signature(
      &entry(
        EntryRole::Item,
        SymbolType::Function,
        "parse",
        "pub fn parse()",
        0,
      ),
      SymbolType::Function,
      false,
      true,
    );
    let public_member = style.member_signature(
      &member(SymbolType::Method, "parse", "parse()", 0, true),
      SymbolType::Method,
    );
    let private_member = style.member_signature(
      &member(SymbolType::Method, "recover", "recover()", 0, false),
      SymbolType::Method,
    );

    assert!(file.contains("\u{1b}[4"));
    assert!(import.contains("\u{1b}["));
    assert!(exported.contains("\u{1b}["));
    assert_ne!(import, "use std::fmt;");
    assert_ne!(exported, "pub fn parse()");
    assert_ne!(public_member, private_member);
    assert!(private_member.contains("\u{1b}["));
  }

  #[test]
  fn keeps_digest_and_names_entries_uncolored() {
    let mut options = options(OutlineView::Names);
    options.use_color = true;
    let mut output = vec![];
    print_text_to(&mut output, &[outline_file()], &options).expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert!(output.contains("Parser"));
    assert!(!output.contains("\u{1b}[34mParser"));
    assert!(output.contains("\u{1b}[1mParser"));

    options.view = OutlineView::Digest;
    let mut output = vec![];
    print_text_to(&mut output, &[outline_file()], &options).expect("text should render");
    let output = String::from_utf8(output).expect("output should be utf8");

    assert!(output.contains("parse"));
    assert!(output.contains("recover"));
    assert!(!output.contains("\u{1b}[32mparse"));
    assert!(!output.contains("\u{1b}[2;32mrecover"));
    assert!(output.contains("\u{1b}[2mrecover"));
  }

  #[test]
  fn suppresses_redundant_item_mode_styles() {
    let import_style = OutlineTextStyle::new(true, OutlineItems::Imports);
    let export_style = OutlineTextStyle::new(true, OutlineItems::Exports);
    let mixed_style = OutlineTextStyle::new(true, OutlineItems::All);
    let import_name = StyledName {
      text: "std::fmt".to_string(),
      is_import: true,
      is_exported: false,
      is_public: true,
    };
    let export_name = StyledName {
      text: "api".to_string(),
      is_import: false,
      is_exported: true,
      is_public: true,
    };

    assert_eq!(import_style.grouped_item_name(&import_name), "std::fmt");
    assert_eq!(export_style.grouped_item_name(&export_name), "api");
    assert_ne!(mixed_style.grouped_item_name(&import_name), "std::fmt");
    assert_ne!(mixed_style.grouped_item_name(&export_name), "api");

    let import_signature = entry(
      EntryRole::Item,
      SymbolType::Module,
      "std::fmt",
      "use std::fmt;",
      0,
    );
    let export_signature = entry(
      EntryRole::Item,
      SymbolType::Function,
      "api",
      "pub fn api()",
      0,
    );

    assert_eq!(
      import_style.entry_signature(&import_signature, SymbolType::Module, true, false),
      "use \u{1b}[36mstd::fmt\u{1b}[0m;"
    );
    assert_eq!(
      export_style.entry_signature(&export_signature, SymbolType::Function, false, true),
      "pub fn \u{1b}[32mapi\u{1b}[0m()"
    );
  }
}
