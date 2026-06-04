use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc;

use anyhow::{Result, anyhow};
use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore, from_str};
use ast_grep_core::Node;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_language::{Language, LanguageExt, SupportLang};
use clap::{Args, Subcommand, ValueEnum};
use ignore::{DirEntry, WalkParallel, WalkState};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::lang::SgLang;
use crate::utils::{InputArgs, read_file};

type SgDoc = StrDoc<SgLang>;
type SgNode<'a> = Node<'a, SgDoc>;

#[derive(Args)]
pub struct OutlineArg {
  #[clap(subcommand)]
  query: OutlineQuery,
}

#[derive(Subcommand, Clone)]
enum OutlineQuery {
  /// Return a compact structural map of files.
  Map(MapArg),
  /// Find symbols by name, kind, role, or regex.
  Find(FindArg),
  /// Return import/dependency edges.
  Imports(ImportsArg),
  /// Return public/exported API symbols.
  Exports(ExportsArg),
  /// Return children of a container symbol.
  Members(MembersArg),
  /// Return the smallest symbol containing a position.
  Container(ContainerArg),
  /// Return structurally related symbols.
  Related(RelatedArg),
  /// Compare outlines before and after a change.
  Diff(DiffArg),
}

#[derive(Args, Clone)]
struct MapArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Filter outline item kinds by LSP SymbolKind name.
  #[clap(long, action = clap::ArgAction::Append)]
  kind: Vec<SymbolKind>,
  /// Maximum nesting depth for tree output.
  #[clap(long, value_name = "NUM")]
  depth: Option<usize>,
}

#[derive(Args, Clone)]
struct FindArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Filter outline item kinds by LSP SymbolKind name.
  #[clap(long, action = clap::ArgAction::Append)]
  kind: Vec<SymbolKind>,
  /// Filter by source role.
  #[clap(long, action = clap::ArgAction::Append)]
  role: Vec<SymbolRole>,
}

#[derive(Args, Clone)]
struct ImportsArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Filter by imported module/package/path.
  #[clap(long, value_name = "MODULE")]
  to: Option<String>,
  /// Flatten import clauses into one row per imported binding.
  #[clap(long)]
  bindings: bool,
}

#[derive(Args, Clone)]
struct ExportsArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Exclude re-export statements without local definitions.
  #[clap(long)]
  definitions_only: bool,
}

#[derive(Args, Clone)]
struct MembersArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Containing symbol name.
  #[clap(long, value_name = "SYMBOL_NAME")]
  of: String,
  /// Disambiguate the containing symbol by LSP SymbolKind.
  #[clap(long, value_name = "KIND")]
  of_kind: Option<SymbolKind>,
  /// Filter member kinds by LSP SymbolKind name.
  #[clap(long, action = clap::ArgAction::Append)]
  kind: Vec<SymbolKind>,
  /// Include recursively nested members.
  #[clap(long)]
  recursive: bool,
}

#[derive(Args, Clone)]
struct ContainerArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Source position to inspect, in 1-based LINE:COLUMN form.
  #[clap(long, value_name = "LINE:COLUMN")]
  at: String,
}

#[derive(Args, Clone)]
struct RelatedArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Seed symbol name.
  #[clap(long, value_name = "SYMBOL_NAME")]
  symbol: Option<String>,
  /// Seed source position, in 1-based LINE:COLUMN form.
  #[clap(long, value_name = "LINE:COLUMN")]
  at: Option<String>,
}

#[derive(Args, Clone)]
struct DiffArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Git revision to compare against.
  #[clap(long, value_name = "REV")]
  base: String,
  /// Compare exported symbols only.
  #[clap(long)]
  exports_only: bool,
}

#[derive(Args, Clone)]
struct OutlineCommonArg {
  /// Language to parse input as. If absent, infer from file path.
  #[clap(short, long)]
  lang: Option<SgLang>,
  /// Output format.
  #[clap(long, default_value = "text", value_name = "FORMAT")]
  format: OutlineFormat,
  /// Approximate maximum records to emit.
  #[clap(long, value_name = "NUM")]
  budget: Option<usize>,
  /// Hard maximum records to emit.
  #[clap(long, value_name = "NUM")]
  max_items: Option<usize>,
  /// Filter symbols/imports/exports by exact name.
  #[clap(long, value_name = "NAME")]
  name: Option<String>,
  /// Filter symbols/imports/exports by substring. Regex support can be added later.
  #[clap(long, value_name = "REGEX")]
  name_regex: Option<String>,
  /// Emit independent records.
  #[clap(long)]
  flat: bool,
  /// Include declaration/signature snippets.
  #[clap(long)]
  signature: bool,
  /// Load additional outline extractor definitions from YAML.
  #[clap(long = "outline-rules", value_name = "FILE", action = clap::ArgAction::Append)]
  outline_rules: Vec<PathBuf>,
  /// Do not load bundled outline extractor definitions.
  #[clap(long)]
  no_default_outline_rules: bool,
  /// Input traversal: paths, globs, ignore behavior, threads.
  #[clap(flatten)]
  input: InputArgs,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[value(rename_all = "camelCase")]
enum OutlineFormat {
  Text,
  Json,
  Jsonl,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[value(rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
enum SymbolKind {
  File = 1,
  Module = 2,
  Namespace = 3,
  Package = 4,
  Class = 5,
  Method = 6,
  Property = 7,
  Field = 8,
  Constructor = 9,
  Enum = 10,
  Interface = 11,
  Function = 12,
  Variable = 13,
  Constant = 14,
  String = 15,
  Number = 16,
  Boolean = 17,
  Array = 18,
  Object = 19,
  Key = 20,
  Null = 21,
  EnumMember = 22,
  Struct = 23,
  Event = 24,
  Operator = 25,
  TypeParameter = 26,
}

impl SymbolKind {
  fn number(self) -> u8 {
    self as u8
  }

  fn name(self) -> &'static str {
    match self {
      SymbolKind::File => "File",
      SymbolKind::Module => "Module",
      SymbolKind::Namespace => "Namespace",
      SymbolKind::Package => "Package",
      SymbolKind::Class => "Class",
      SymbolKind::Method => "Method",
      SymbolKind::Property => "Property",
      SymbolKind::Field => "Field",
      SymbolKind::Constructor => "Constructor",
      SymbolKind::Enum => "Enum",
      SymbolKind::Interface => "Interface",
      SymbolKind::Function => "Function",
      SymbolKind::Variable => "Variable",
      SymbolKind::Constant => "Constant",
      SymbolKind::String => "String",
      SymbolKind::Number => "Number",
      SymbolKind::Boolean => "Boolean",
      SymbolKind::Array => "Array",
      SymbolKind::Object => "Object",
      SymbolKind::Key => "Key",
      SymbolKind::Null => "Null",
      SymbolKind::EnumMember => "EnumMember",
      SymbolKind::Struct => "Struct",
      SymbolKind::Event => "Event",
      SymbolKind::Operator => "Operator",
      SymbolKind::TypeParameter => "TypeParameter",
    }
  }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[value(rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
enum SymbolRole {
  Definition,
  Import,
  Export,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Position {
  line: usize,
  column: usize,
  byte: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineRange {
  start: Position,
  end: Position,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineItem {
  name: Option<String>,
  kind: u8,
  kind_name: &'static str,
  role: SymbolRole,
  range: OutlineRange,
  selection_range: OutlineRange,
  #[serde(skip_serializing_if = "Option::is_none")]
  signature: Option<String>,
  exported: bool,
  node_kind: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  children: Vec<OutlineItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineFile {
  path: String,
  language: String,
  items: Vec<OutlineItem>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineContainer {
  name: Option<String>,
  kind: u8,
  kind_name: &'static str,
  range: OutlineRange,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineRecord {
  path: String,
  language: String,
  query: &'static str,
  symbol: OutlineFlatSymbol,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineFlatSymbol {
  name: Option<String>,
  kind: u8,
  kind_name: &'static str,
  role: SymbolRole,
  range: OutlineRange,
  selection_range: OutlineRange,
  #[serde(skip_serializing_if = "Option::is_none")]
  signature: Option<String>,
  exported: bool,
  node_kind: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  container: Option<OutlineContainer>,
  #[serde(skip_serializing_if = "Option::is_none")]
  score: Option<f32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineDiff {
  base: String,
  added: Vec<DiffRecord>,
  removed: Vec<DiffRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffRecord {
  change: DiffChange,
  record: OutlineRecord,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum DiffChange {
  Added,
  Removed,
}

struct RuleSpec {
  kind: SymbolKind,
  role: SymbolRole,
  name: NameSource,
  exported: ExportPolicy,
  matcher: RuleCore,
}

struct OutlineCatalog {
  extractors: Vec<SerializableOutlineExtractor>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableOutlineExtractor {
  id: String,
  language: SgLang,
  kind: SymbolKind,
  role: SymbolRole,
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  exported: Option<String>,
  #[serde(flatten)]
  core: SerializableRuleCore,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OutlineExtractorFile {
  Wrapped {
    extractors: Vec<SerializableOutlineExtractor>,
  },
  List(Vec<SerializableOutlineExtractor>),
}

impl OutlineExtractorFile {
  fn into_extractors(self) -> Vec<SerializableOutlineExtractor> {
    match self {
      Self::Wrapped { extractors } => extractors,
      Self::List(extractors) => extractors,
    }
  }
}

#[derive(Clone)]
enum NameSource {
  Auto,
  Text,
  Field(String),
  MetaVar(String),
}

#[derive(Clone)]
enum ExportPolicy {
  Auto,
  Always,
  Never,
  NameUppercase,
  TextPrefix(String),
  AncestorKind(String),
}

const DEFAULT_OUTLINE_RULES: &str = r#"
extractors:
  - id: rust-use
    language: Rust
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: use_declaration }
  - id: rust-mod
    language: Rust
    kind: module
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: mod_item }
  - id: rust-function
    language: Rust
    kind: function
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: function_item }
  - id: rust-struct
    language: Rust
    kind: struct
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: struct_item }
  - id: rust-enum
    language: Rust
    kind: enum
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: enum_item }
  - id: rust-trait
    language: Rust
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: trait_item }
  - id: rust-type
    language: Rust
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: type_item }
  - id: rust-const
    language: Rust
    kind: constant
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: const_item }
  - id: rust-static
    language: Rust
    kind: variable
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: static_item }
  - id: rust-impl
    language: Rust
    kind: object
    role: definition
    name: auto
    exported: never
    rule: { kind: impl_item }
  - id: rust-field
    language: Rust
    kind: field
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: field_declaration }
  - id: rust-enum-variant
    language: Rust
    kind: enumMember
    role: definition
    name: auto
    exported: never
    rule: { kind: enum_variant }

  - id: ts-import
    language: TypeScript
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: ts-re-export
    language: TypeScript
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*|type\s+\{)'
  - id: ts-function
    language: TypeScript
    kind: function
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: function_declaration }
  - id: ts-class
    language: TypeScript
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: class_declaration }
  - id: ts-interface
    language: TypeScript
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: interface_declaration }
  - id: ts-type
    language: TypeScript
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: type_alias_declaration }
  - id: ts-method
    language: TypeScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule: { kind: method_definition }
  - id: ts-field
    language: TypeScript
    kind: field
    role: definition
    name: auto
    exported: never
    rule: { kind: public_field_definition }
  - id: ts-const
    language: TypeScript
    kind: constant
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
  - id: ts-variable
    language: TypeScript
    kind: variable
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*(let|var)\b'

  - id: tsx-import
    language: Tsx
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: tsx-re-export
    language: Tsx
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*|type\s+\{)'
  - id: tsx-function
    language: Tsx
    kind: function
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: function_declaration }
  - id: tsx-class
    language: Tsx
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: class_declaration }
  - id: tsx-interface
    language: Tsx
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: interface_declaration }
  - id: tsx-type
    language: Tsx
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: type_alias_declaration }
  - id: tsx-method
    language: Tsx
    kind: method
    role: definition
    name: field:name
    exported: never
    rule: { kind: method_definition }
  - id: tsx-field
    language: Tsx
    kind: field
    role: definition
    name: auto
    exported: never
    rule: { kind: public_field_definition }
  - id: tsx-const
    language: Tsx
    kind: constant
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
  - id: tsx-variable
    language: Tsx
    kind: variable
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*(let|var)\b'

  - id: js-import
    language: JavaScript
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: js-re-export
    language: JavaScript
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*)'
  - id: js-function
    language: JavaScript
    kind: function
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: function_declaration }
  - id: js-class
    language: JavaScript
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: class_declaration }
  - id: js-method
    language: JavaScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule: { kind: method_definition }
  - id: js-field
    language: JavaScript
    kind: field
    role: definition
    name: auto
    exported: never
    rule: { kind: public_field_definition }
  - id: js-const
    language: JavaScript
    kind: constant
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
  - id: js-variable
    language: JavaScript
    kind: variable
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*(let|var)\b'

  - id: py-import
    language: Python
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: py-from-import
    language: Python
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_from_statement }
  - id: py-function
    language: Python
    kind: function
    role: definition
    name: field:name
    exported: never
    rule: { kind: function_definition }
  - id: py-class
    language: Python
    kind: class
    role: definition
    name: field:name
    exported: never
    rule: { kind: class_definition }

  - id: go-package
    language: Go
    kind: package
    role: definition
    name: auto
    exported: never
    rule: { kind: package_clause }
  - id: go-import
    language: Go
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_declaration }
  - id: go-function
    language: Go
    kind: function
    role: definition
    name: field:name
    exported: nameUppercase
    rule: { kind: function_declaration }
  - id: go-method
    language: Go
    kind: method
    role: definition
    name: field:name
    exported: nameUppercase
    rule: { kind: method_declaration }
  - id: go-type
    language: Go
    kind: interface
    role: definition
    name: auto
    exported: nameUppercase
    rule: { kind: type_declaration }
  - id: go-const
    language: Go
    kind: constant
    role: definition
    name: auto
    exported: nameUppercase
    rule: { kind: const_declaration }
  - id: go-var
    language: Go
    kind: variable
    role: definition
    name: auto
    exported: nameUppercase
    rule: { kind: var_declaration }
"#;

pub fn run_outline(arg: OutlineArg) -> Result<ExitCode> {
  let query = arg.query;
  if let OutlineQuery::Diff(arg) = &query {
    return run_diff(&query, arg);
  }
  if let OutlineQuery::Related(arg) = &query {
    return run_related(&query, arg);
  }
  let common = query.common();
  let mut files = if common.input.stdin {
    vec![outline_stdin(&query)?]
  } else {
    outline_paths(&query)?
  };
  apply_query(&query, &mut files);
  print_outline(&query, files)
}

impl OutlineQuery {
  fn common(&self) -> &OutlineCommonArg {
    match self {
      OutlineQuery::Map(arg) => &arg.common,
      OutlineQuery::Find(arg) => &arg.common,
      OutlineQuery::Imports(arg) => &arg.common,
      OutlineQuery::Exports(arg) => &arg.common,
      OutlineQuery::Members(arg) => &arg.common,
      OutlineQuery::Container(arg) => &arg.common,
      OutlineQuery::Related(arg) => &arg.common,
      OutlineQuery::Diff(arg) => &arg.common,
    }
  }

  fn query_name(&self) -> &'static str {
    match self {
      OutlineQuery::Map(_) => "map",
      OutlineQuery::Find(_) => "find",
      OutlineQuery::Imports(_) => "imports",
      OutlineQuery::Exports(_) => "exports",
      OutlineQuery::Members(_) => "members",
      OutlineQuery::Container(_) => "container",
      OutlineQuery::Related(_) => "related",
      OutlineQuery::Diff(_) => "diff",
    }
  }
}

fn outline_stdin(query: &OutlineQuery) -> Result<OutlineFile> {
  let common = query.common();
  let catalog = load_outline_catalog(common)?;
  let lang = common
    .lang
    .ok_or_else(|| anyhow!("--stdin requires --lang"))?;
  let src = std::io::read_to_string(std::io::stdin())?;
  extract_outline("STDIN".into(), lang, &src, common, &catalog)
}

fn outline_paths(query: &OutlineQuery) -> Result<Vec<OutlineFile>> {
  let common = query.common();
  let walker = build_walk(common)?;
  let (tx, rx) = mpsc::channel();
  let common = Arc::new(common.clone());
  let catalog = Arc::new(load_outline_catalog(&common)?);
  std::thread::spawn(move || {
    walker.run(|| {
      let tx = tx.clone();
      let common = common.clone();
      let catalog = catalog.clone();
      Box::new(move |result| {
        let Some(path) = filter_entry(result) else {
          return WalkState::Continue;
        };
        let Some(lang) = common.lang.or_else(|| SgLang::from_path(&path)) else {
          return WalkState::Continue;
        };
        let Ok(src) = read_file(&path) else {
          return WalkState::Continue;
        };
        let Ok(outline) = extract_outline(path, lang, &src, &common, &catalog) else {
          return WalkState::Continue;
        };
        if tx.send(outline).is_err() {
          return WalkState::Quit;
        }
        WalkState::Continue
      })
    });
  });
  let mut files = rx.into_iter().collect::<Vec<_>>();
  files.sort_by(|a, b| a.path.cmp(&b.path));
  Ok(files)
}

fn run_diff(query: &OutlineQuery, arg: &DiffArg) -> Result<ExitCode> {
  let mut current = outline_paths(query)?;
  let catalog = load_outline_catalog(&arg.common)?;
  for file in &mut current {
    file.items = filter_diff_items(std::mem::take(&mut file.items), arg);
  }
  current.retain(|file| !file.items.is_empty());

  let mut base = vec![];
  for file in &current {
    let path = PathBuf::from(&file.path);
    let Some(lang) = arg.common.lang.or_else(|| SgLang::from_path(&path)) else {
      continue;
    };
    let Some(src) = read_git_file(&arg.base, &file.path) else {
      continue;
    };
    let mut outline = extract_outline(path, lang, &src, &arg.common, &catalog)?;
    outline.items = filter_diff_items(outline.items, arg);
    if !outline.items.is_empty() {
      base.push(outline);
    }
  }

  let diff = diff_records(
    &arg.base,
    flatten_files(query, &base),
    flatten_files(query, &current),
  );
  print_diff(query, diff)
}

fn run_related(query: &OutlineQuery, arg: &RelatedArg) -> Result<ExitCode> {
  let mut files = outline_paths(query)?;
  apply_common_only(&mut files, &arg.common);
  let seeds = related_seeds(&files, arg);
  if seeds.is_empty() {
    return print_related(query, vec![]);
  }
  let seed_paths = seeds
    .iter()
    .map(|seed| seed.path.clone())
    .collect::<Vec<_>>();
  let seed_names = seeds
    .iter()
    .filter_map(|seed| seed.symbol.name.clone())
    .collect::<Vec<_>>();
  let mut related = vec![];
  for mut record in flatten_files(query, &files) {
    if seeds
      .iter()
      .any(|seed| record_key(seed) == record_key(&record))
    {
      continue;
    }
    if let Some((score, reason)) = related_score(&record, &seed_names, &seed_paths) {
      record.symbol.score = Some(score);
      record.symbol.reason = Some(reason.to_string());
      related.push(record);
    }
  }
  related.sort_by(|a, b| {
    b.symbol
      .score
      .partial_cmp(&a.symbol.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| a.path.cmp(&b.path))
  });
  if let Some(limit) = arg.common.max_items.or(arg.common.budget) {
    related.truncate(limit);
  }
  print_related(query, related)
}

fn apply_common_only(files: &mut Vec<OutlineFile>, common: &OutlineCommonArg) {
  for file in files {
    file.items = filter_tree(std::mem::take(&mut file.items), |item| {
      common_matches(item, common)
    });
  }
}

fn related_seeds(files: &[OutlineFile], arg: &RelatedArg) -> Vec<OutlineRecord> {
  let mut seeds = vec![];
  if let Some(symbol) = &arg.symbol {
    for record in flatten_files(&OutlineQuery::Related(arg.clone()), files) {
      if record.symbol.name.as_deref() == Some(symbol.as_str()) {
        seeds.push(record);
      }
    }
  }
  if let Some(at) = &arg.at
    && let Some(point) = parse_line_col(at)
  {
    for file in files {
      let mut containers = vec![];
      for item in &file.items {
        collect_container_records(file, item, point, None, &mut containers);
      }
      if let Some(seed) = containers.into_iter().min_by_key(|record| {
        record
          .symbol
          .range
          .end
          .byte
          .saturating_sub(record.symbol.range.start.byte)
      }) {
        seeds.push(seed);
      }
    }
  }
  seeds
}

fn collect_container_records(
  file: &OutlineFile,
  item: &OutlineItem,
  point: (usize, usize),
  container: Option<OutlineContainer>,
  ret: &mut Vec<OutlineRecord>,
) {
  if !range_contains_point(&item.range, point) {
    return;
  }
  let current_container = Some(OutlineContainer {
    name: item.name.clone(),
    kind: item.kind,
    kind_name: item.kind_name,
    range: item.range.clone(),
  });
  ret.push(OutlineRecord {
    path: file.path.clone(),
    language: file.language.clone(),
    query: "related",
    symbol: flat_symbol(item, container),
  });
  for child in &item.children {
    collect_container_records(file, child, point, current_container.clone(), ret);
  }
}

fn related_score(
  record: &OutlineRecord,
  seed_names: &[String],
  seed_paths: &[String],
) -> Option<(f32, &'static str)> {
  let name = record.symbol.name.as_deref().unwrap_or_default();
  let signature = record.symbol.signature.as_deref().unwrap_or_default();
  let path = record.path.as_str();
  for seed in seed_names {
    if name == seed {
      return Some((0.95, "same-name-symbol"));
    }
    if !seed.is_empty() && (name.contains(seed) || signature.contains(seed)) {
      return Some((0.80, "name-proximity"));
    }
    if record.symbol.role == SymbolRole::Import && item_text_contains(&record.symbol, seed) {
      return Some((0.75, "imports-seed-module"));
    }
    if (record.symbol.exported || record.symbol.role == SymbolRole::Export)
      && item_text_contains(&record.symbol, seed)
    {
      return Some((0.70, "exported-seed-name"));
    }
    let seed_lower = seed.to_lowercase();
    if (path.contains("test") || path.contains("spec"))
      && (name.to_lowercase().contains(&seed_lower)
        || signature.to_lowercase().contains(&seed_lower))
    {
      return Some((0.60, "test-name-match"));
    }
  }
  if seed_paths.iter().any(|seed_path| seed_path == path) {
    return Some((0.50, "same-file-symbol"));
  }
  None
}

fn item_text_contains(symbol: &OutlineFlatSymbol, needle: &str) -> bool {
  symbol
    .name
    .as_ref()
    .is_some_and(|name| name.contains(needle))
    || symbol
      .signature
      .as_ref()
      .is_some_and(|signature| signature.contains(needle))
}

fn read_git_file(base: &str, path: &str) -> Option<String> {
  let spec = format!("{base}:{path}");
  let output = Command::new("git").args(["show", &spec]).output().ok()?;
  output
    .status
    .success()
    .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn filter_diff_items(items: Vec<OutlineItem>, arg: &DiffArg) -> Vec<OutlineItem> {
  filter_tree(items, |item| {
    (!arg.exports_only || item.exported || item.role == SymbolRole::Export)
      && common_matches(item, &arg.common)
  })
}

fn diff_records(
  base: &str,
  old_records: Vec<OutlineRecord>,
  new_records: Vec<OutlineRecord>,
) -> OutlineDiff {
  let old = old_records
    .into_iter()
    .map(|record| (record_key(&record), record))
    .collect::<BTreeMap<_, _>>();
  let new = new_records
    .into_iter()
    .map(|record| (record_key(&record), record))
    .collect::<BTreeMap<_, _>>();
  let added = new
    .iter()
    .filter(|(key, _)| !old.contains_key(*key))
    .map(|(_, record)| DiffRecord {
      change: DiffChange::Added,
      record: record.clone(),
    })
    .collect();
  let removed = old
    .iter()
    .filter(|(key, _)| !new.contains_key(*key))
    .map(|(_, record)| DiffRecord {
      change: DiffChange::Removed,
      record: record.clone(),
    })
    .collect();
  OutlineDiff {
    base: base.to_string(),
    added,
    removed,
  }
}

fn record_key(record: &OutlineRecord) -> String {
  let symbol = &record.symbol;
  let container = symbol
    .container
    .as_ref()
    .and_then(|c| c.name.as_deref())
    .unwrap_or("");
  format!(
    "{}\0{}\0{}\0{:?}\0{}",
    record.path,
    symbol.name.as_deref().unwrap_or(""),
    symbol.kind,
    symbol.role,
    container
  )
}

fn build_walk(common: &OutlineCommonArg) -> Result<WalkParallel> {
  if let Some(lang) = common.lang {
    common.input.walk_lang(lang)
  } else {
    common.input.walk()
  }
}

fn filter_entry(result: Result<DirEntry, ignore::Error>) -> Option<PathBuf> {
  let entry = match result {
    Ok(entry) => entry,
    Err(err) => {
      eprintln!("ERROR: {err}");
      return None;
    }
  };
  if !entry.file_type()?.is_file() {
    return None;
  }
  let path = entry.into_path();
  path
    .strip_prefix("./")
    .map_or_else(|_| Some(path.clone()), |p| Some(p.to_path_buf()))
}

fn extract_outline(
  path: PathBuf,
  lang: SgLang,
  src: &str,
  common: &OutlineCommonArg,
  catalog: &OutlineCatalog,
) -> Result<OutlineFile> {
  let grep = lang.ast_grep(src);
  let root = grep.root();
  let mut items = vec![];
  for spec in outline_rules(lang, catalog)? {
    for matched in root.find_all(&spec.matcher) {
      if let Some(item) = make_item(&matched, lang, &spec, common.signature) {
        items.push(item);
      }
    }
  }
  dedup_items(&mut items);
  items.sort_by_key(|i| (i.range.start.byte, Reverse(i.range.end.byte)));
  let items = nest_items(items);
  Ok(OutlineFile {
    path: path.to_string_lossy().to_string(),
    language: lang.to_string(),
    items,
  })
}

fn load_outline_catalog(common: &OutlineCommonArg) -> Result<OutlineCatalog> {
  let mut extractors = vec![];
  if !common.no_default_outline_rules {
    extractors.extend(parse_outline_extractors(DEFAULT_OUTLINE_RULES)?);
  }
  for path in &common.outline_rules {
    extractors.extend(read_outline_extractors(path)?);
  }
  Ok(OutlineCatalog { extractors })
}

fn outline_rules(lang: SgLang, catalog: &OutlineCatalog) -> Result<Vec<RuleSpec>> {
  compile_outline_rules(lang, catalog.extractors.clone())
}

fn read_outline_extractors(path: &Path) -> Result<Vec<SerializableOutlineExtractor>> {
  let yaml = read_file(path)?;
  parse_outline_extractors(&yaml)
}

fn parse_outline_extractors(yaml: &str) -> Result<Vec<SerializableOutlineExtractor>> {
  let file: OutlineExtractorFile = from_str(yaml)?;
  Ok(file.into_extractors())
}

fn compile_outline_rules(
  lang: SgLang,
  extractors: Vec<SerializableOutlineExtractor>,
) -> Result<Vec<RuleSpec>> {
  let env = DeserializeEnv::new(lang);
  extractors
    .into_iter()
    .filter(|spec| spec.language == lang)
    .map(|spec| {
      let _id = &spec.id;
      let matcher = spec.core.get_matcher(env.clone())?;
      Ok(RuleSpec {
        kind: spec.kind,
        role: spec.role,
        name: parse_name_source(spec.name),
        exported: parse_export_policy(spec.exported),
        matcher,
      })
    })
    .collect()
}

fn parse_name_source(source: Option<String>) -> NameSource {
  let Some(source) = source else {
    return NameSource::Auto;
  };
  if source == "auto" {
    NameSource::Auto
  } else if source == "text" {
    NameSource::Text
  } else if let Some(field) = source.strip_prefix("field:") {
    NameSource::Field(field.into())
  } else {
    NameSource::MetaVar(source.trim_start_matches('$').into())
  }
}

fn parse_export_policy(policy: Option<String>) -> ExportPolicy {
  let Some(policy) = policy else {
    return ExportPolicy::Auto;
  };
  match policy.as_str() {
    "always" | "true" => ExportPolicy::Always,
    "never" | "false" => ExportPolicy::Never,
    "auto" => ExportPolicy::Auto,
    "nameUppercase" => ExportPolicy::NameUppercase,
    _ => {
      if let Some(prefix) = policy.strip_prefix("textPrefix:") {
        ExportPolicy::TextPrefix(prefix.into())
      } else if let Some(kind) = policy.strip_prefix("ancestorKind:") {
        ExportPolicy::AncestorKind(kind.into())
      } else {
        ExportPolicy::Auto
      }
    }
  }
}

fn make_item(
  matched: &ast_grep_core::NodeMatch<SgDoc>,
  lang: SgLang,
  spec: &RuleSpec,
  include_signature: bool,
) -> Option<OutlineItem> {
  let node = matched.get_node();
  let (name, selection_range) = resolve_name(matched, lang, spec);
  if matches!(spec.role, SymbolRole::Definition) && name.is_none() {
    return None;
  }
  let exported = is_exported(node, &spec.exported, spec.role, name.as_deref());
  let kind = spec.kind;
  Some(OutlineItem {
    name,
    kind: kind.number(),
    kind_name: kind.name(),
    role: spec.role,
    range: node_range(node),
    selection_range: selection_range.unwrap_or_else(|| node_range(node)),
    signature: include_signature.then(|| signature(node)),
    exported,
    node_kind: node.kind().to_string(),
    children: vec![],
  })
}

fn resolve_name(
  matched: &ast_grep_core::NodeMatch<SgDoc>,
  lang: SgLang,
  spec: &RuleSpec,
) -> (Option<String>, Option<OutlineRange>) {
  let node = matched.get_node();
  match &spec.name {
    NameSource::Text => return (Some(import_export_name(node)), None),
    NameSource::Field(field) => {
      if let Some(name) = node.field(field) {
        return (
          Some(name.text().trim().to_string()),
          Some(node_range(&name)),
        );
      }
    }
    NameSource::MetaVar(var) => {
      if let Some(name) = matched.get_env().get_match(var) {
        return (Some(name.text().trim().to_string()), Some(node_range(name)));
      }
    }
    NameSource::Auto => {}
  }
  if matches!(spec.role, SymbolRole::Import | SymbolRole::Export) {
    return (Some(import_export_name(node)), None);
  }
  if let Some(name) = node.field("name") {
    return (
      Some(name.text().trim().to_string()),
      Some(node_range(&name)),
    );
  }
  if node.kind().as_ref() == "lexical_declaration" || node.kind().as_ref() == "variable_declaration"
  {
    if let Some(name) = node.dfs().find(|n| {
      matches!(
        n.kind().as_ref(),
        "identifier" | "shorthand_property_identifier_pattern"
      )
    }) {
      return (
        Some(name.text().trim().to_string()),
        Some(node_range(&name)),
      );
    }
  }
  if lang == SgLang::Builtin(SupportLang::Go)
    && let Some(name) = node.dfs().find(|n| n.kind().as_ref() == "identifier")
  {
    return (
      Some(name.text().trim().to_string()),
      Some(node_range(&name)),
    );
  }
  if node.kind().as_ref() == "impl_item" {
    let text = node.text();
    let name = text
      .trim_start()
      .strip_prefix("impl")
      .map(str::trim)
      .and_then(|s| s.split([' ', '{', '<']).find(|s| !s.is_empty()))
      .map(str::to_string);
    return (name, None);
  }
  if let Some(name) = node.dfs().find(is_name_like_node) {
    return (
      Some(name.text().trim().to_string()),
      Some(node_range(&name)),
    );
  }
  (None, None)
}

fn is_name_like_node(node: &SgNode<'_>) -> bool {
  matches!(
    node.kind().as_ref(),
    "identifier"
      | "type_identifier"
      | "field_identifier"
      | "property_identifier"
      | "shorthand_property_identifier"
      | "constant"
  )
}

fn import_export_name(node: &SgNode<'_>) -> String {
  let text = node.text();
  let text = text.trim();
  if let Some(quoted) = extract_quoted(text) {
    quoted
  } else {
    text
      .lines()
      .next()
      .unwrap_or(text)
      .trim()
      .trim_start_matches("use ")
      .trim_start_matches("export ")
      .trim_end_matches(';')
      .trim()
      .to_string()
  }
}

fn extract_quoted(text: &str) -> Option<String> {
  for quote in ['"', '\'', '`'] {
    let start = text.find(quote)?;
    let rest = &text[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    if end > 0 {
      return Some(rest[..end].to_string());
    }
  }
  None
}

fn is_exported(
  node: &SgNode<'_>,
  policy: &ExportPolicy,
  role: SymbolRole,
  name: Option<&str>,
) -> bool {
  if matches!(role, SymbolRole::Export) {
    return true;
  }
  match policy {
    ExportPolicy::Always => true,
    ExportPolicy::Never => false,
    ExportPolicy::NameUppercase => name
      .and_then(|n| n.chars().next())
      .is_some_and(char::is_uppercase),
    ExportPolicy::TextPrefix(prefix) => node.text().trim_start().starts_with(prefix),
    ExportPolicy::AncestorKind(kind) => node.ancestors().any(|n| n.kind().as_ref() == kind),
    ExportPolicy::Auto => false,
  }
}

fn signature(node: &SgNode<'_>) -> String {
  node
    .text()
    .lines()
    .next()
    .unwrap_or_default()
    .trim()
    .to_string()
}

fn node_range(node: &SgNode<'_>) -> OutlineRange {
  let start = node.start_pos();
  let end = node.end_pos();
  OutlineRange {
    start: Position {
      line: start.line(),
      column: start.column(node),
      byte: node.range().start,
    },
    end: Position {
      line: end.line(),
      column: end.column(node),
      byte: node.range().end,
    },
  }
}

fn dedup_items(items: &mut Vec<OutlineItem>) {
  items.sort_by_key(|i| {
    (
      i.range.start.byte,
      i.range.end.byte,
      i.kind,
      i.role as u8,
      i.name.clone(),
    )
  });
  items.dedup_by(|a, b| {
    a.range.start.byte == b.range.start.byte
      && a.range.end.byte == b.range.end.byte
      && a.kind == b.kind
      && a.role == b.role
      && a.name == b.name
  });
}

fn nest_items(items: Vec<OutlineItem>) -> Vec<OutlineItem> {
  let mut roots = vec![];
  for item in items {
    insert_nested(&mut roots, item);
  }
  roots
}

fn insert_nested(items: &mut Vec<OutlineItem>, item: OutlineItem) {
  for parent in items.iter_mut().rev() {
    if contains_range(parent, &item) {
      insert_nested(&mut parent.children, item);
      return;
    }
  }
  items.push(item);
}

fn contains_range(parent: &OutlineItem, child: &OutlineItem) -> bool {
  parent.range.start.byte <= child.range.start.byte
    && child.range.end.byte <= parent.range.end.byte
    && (parent.range.start.byte, parent.range.end.byte)
      != (child.range.start.byte, child.range.end.byte)
}

fn apply_query(query: &OutlineQuery, files: &mut Vec<OutlineFile>) {
  let common = query.common();
  for file in files.iter_mut() {
    file.items = filter_items(std::mem::take(&mut file.items), query, common);
  }
  files.retain(|file| !file.items.is_empty() || matches!(query, OutlineQuery::Map(_)));
}

fn filter_items(
  items: Vec<OutlineItem>,
  query: &OutlineQuery,
  common: &OutlineCommonArg,
) -> Vec<OutlineItem> {
  match query {
    OutlineQuery::Map(arg) => {
      let mut items = filter_tree(items, |item| {
        kind_matches(item, &arg.kind) && common_matches(item, common)
      });
      if let Some(depth) = arg.depth {
        trim_depth(&mut items, depth);
      }
      items
    }
    OutlineQuery::Find(arg) => filter_tree(items, |item| {
      kind_matches(item, &arg.kind) && role_matches(item, &arg.role) && common_matches(item, common)
    }),
    OutlineQuery::Imports(arg) => {
      let imports = filter_tree(items, |item| {
        item.role == SymbolRole::Import
          && common_matches(item, common)
          && arg
            .to
            .as_ref()
            .is_none_or(|to| item_name_contains(item, to))
      });
      if arg.bindings {
        imports.into_iter().flat_map(expand_bindings).collect()
      } else {
        imports
      }
    }
    OutlineQuery::Exports(arg) => filter_tree(items, |item| {
      (item.exported || (!arg.definitions_only && item.role == SymbolRole::Export))
        && common_matches(item, common)
    }),
    OutlineQuery::Members(arg) => filter_members(items, arg, common),
    OutlineQuery::Container(arg) => filter_container(items, arg),
    OutlineQuery::Related(_) => items,
    OutlineQuery::Diff(_) => items,
  }
}

fn trim_depth(items: &mut [OutlineItem], depth: usize) {
  if depth == 0 {
    for item in items {
      item.children.clear();
    }
    return;
  }
  for item in items {
    if depth == 1 {
      item.children.clear();
    } else {
      trim_depth(&mut item.children, depth - 1);
    }
  }
}

fn filter_tree(
  items: Vec<OutlineItem>,
  pred: impl Copy + Fn(&OutlineItem) -> bool,
) -> Vec<OutlineItem> {
  items
    .into_iter()
    .filter_map(|mut item| {
      item.children = filter_tree(item.children, pred);
      if pred(&item) { Some(item) } else { None }
    })
    .collect()
}

fn filter_members(
  items: Vec<OutlineItem>,
  arg: &MembersArg,
  common: &OutlineCommonArg,
) -> Vec<OutlineItem> {
  let mut ret = vec![];
  for item in items {
    collect_members(item, arg, common, &mut ret);
  }
  ret
}

fn collect_members(
  item: OutlineItem,
  arg: &MembersArg,
  common: &OutlineCommonArg,
  ret: &mut Vec<OutlineItem>,
) {
  let is_container = item.name.as_deref() == Some(arg.of.as_str())
    && arg.of_kind.is_none_or(|kind| item.kind == kind.number());
  if is_container {
    let children = if arg.recursive {
      flatten_items(item.children)
    } else {
      item.children
    };
    ret.extend(
      children
        .into_iter()
        .filter(|child| kind_matches(child, &arg.kind) && common_matches(child, common)),
    );
  } else {
    for child in item.children {
      collect_members(child, arg, common, ret);
    }
  }
}

fn flatten_items(items: Vec<OutlineItem>) -> Vec<OutlineItem> {
  let mut ret = vec![];
  for mut item in items {
    let children = std::mem::take(&mut item.children);
    ret.push(item);
    ret.extend(flatten_items(children));
  }
  ret
}

fn filter_container(items: Vec<OutlineItem>, arg: &ContainerArg) -> Vec<OutlineItem> {
  let Some(point) = parse_line_col(&arg.at) else {
    return vec![];
  };
  let mut best = vec![];
  for item in items {
    if let Some(chain) = container_chain(item, point) {
      if chain_len(&chain) > chain_len(&best) {
        best = chain;
      }
    }
  }
  best
}

fn container_chain(mut item: OutlineItem, point: (usize, usize)) -> Option<Vec<OutlineItem>> {
  if !range_contains_point(&item.range, point) {
    return None;
  }
  let mut best_child = vec![];
  for child in std::mem::take(&mut item.children) {
    if let Some(chain) = container_chain(child, point) {
      if chain_len(&chain) > chain_len(&best_child) {
        best_child = chain;
      }
    }
  }
  if let Some(child) = best_child.into_iter().next() {
    item.children = vec![child];
  }
  Some(vec![item])
}

fn chain_len(items: &[OutlineItem]) -> usize {
  fn item_len(item: &OutlineItem) -> usize {
    1 + item.children.first().map_or(0, item_len)
  }
  items.first().map_or(0, item_len)
}

fn range_contains_point(range: &OutlineRange, point: (usize, usize)) -> bool {
  let (line, column) = point;
  let starts_before =
    range.start.line < line || (range.start.line == line && range.start.column <= column);
  let ends_after = range.end.line > line || (range.end.line == line && range.end.column >= column);
  starts_before && ends_after
}

fn parse_line_col(input: &str) -> Option<(usize, usize)> {
  let (line, column) = input.split_once(':')?;
  let line = line.parse::<usize>().ok()?.checked_sub(1)?;
  let column = column.parse::<usize>().ok()?.checked_sub(1)?;
  Some((line, column))
}

fn kind_matches(item: &OutlineItem, kinds: &[SymbolKind]) -> bool {
  kinds.is_empty() || kinds.iter().any(|kind| item.kind == kind.number())
}

fn role_matches(item: &OutlineItem, roles: &[SymbolRole]) -> bool {
  roles.is_empty() || roles.contains(&item.role)
}

fn common_matches(item: &OutlineItem, common: &OutlineCommonArg) -> bool {
  common
    .name
    .as_ref()
    .is_none_or(|name| item.name.as_deref() == Some(name.as_str()))
    && common
      .name_regex
      .as_ref()
      .is_none_or(|regex| item_name_regex(item, regex))
}

fn item_name_contains(item: &OutlineItem, needle: &str) -> bool {
  let normalized = needle.replace('-', "_");
  item
    .name
    .as_ref()
    .is_some_and(|name| name.contains(needle) || name.contains(&normalized))
    || item
      .signature
      .as_ref()
      .is_some_and(|signature| signature.contains(needle) || signature.contains(&normalized))
}

fn item_name_regex(item: &OutlineItem, pattern: &str) -> bool {
  let Ok(regex) = Regex::new(pattern) else {
    return false;
  };
  item.name.as_ref().is_some_and(|name| regex.is_match(name))
    || item
      .signature
      .as_ref()
      .is_some_and(|signature| regex.is_match(signature))
}

fn expand_bindings(item: OutlineItem) -> Vec<OutlineItem> {
  let Some(name) = &item.name else {
    return vec![item];
  };
  let Some(start) = name.find('{') else {
    return vec![item];
  };
  let Some(end) = name.rfind('}') else {
    return vec![item];
  };
  if end <= start {
    return vec![item];
  }
  let prefix = name[..start].trim().trim_end_matches("::").trim();
  let bindings = name[start + 1..end]
    .split(',')
    .map(str::trim)
    .filter(|binding| !binding.is_empty())
    .map(|binding| {
      let mut item = item.clone();
      item.name = Some(if prefix.is_empty() {
        binding.to_string()
      } else {
        format!("{prefix}::{binding}")
      });
      item.children.clear();
      item
    })
    .collect::<Vec<_>>();
  if bindings.is_empty() {
    vec![item]
  } else {
    bindings
  }
}

fn print_outline(query: &OutlineQuery, mut files: Vec<OutlineFile>) -> Result<ExitCode> {
  let common = query.common();
  enforce_limit(&mut files, common.max_items.or(common.budget));
  match common.format {
    OutlineFormat::Text => print_text(&files),
    OutlineFormat::Json => {
      if common.flat {
        let records = flatten_files(query, &files);
        println!("{}", serde_json::to_string_pretty(&records)?);
      } else {
        println!("{}", serde_json::to_string_pretty(&files)?);
      }
    }
    OutlineFormat::Jsonl => {
      for record in flatten_files(query, &files) {
        println!("{}", serde_json::to_string(&record)?);
      }
    }
  }
  Ok(ExitCode::SUCCESS)
}

fn print_diff(query: &OutlineQuery, diff: OutlineDiff) -> Result<ExitCode> {
  match query.common().format {
    OutlineFormat::Text => {
      println!("base {}", diff.base);
      for record in &diff.added {
        print_diff_text_record(record);
      }
      for record in &diff.removed {
        print_diff_text_record(record);
      }
    }
    OutlineFormat::Json => {
      println!("{}", serde_json::to_string_pretty(&diff)?);
    }
    OutlineFormat::Jsonl => {
      for record in diff.added {
        println!("{}", serde_json::to_string(&record)?);
      }
      for record in diff.removed {
        println!("{}", serde_json::to_string(&record)?);
      }
    }
  }
  Ok(ExitCode::SUCCESS)
}

fn print_diff_text_record(record: &DiffRecord) {
  let sign = match record.change {
    DiffChange::Added => "+",
    DiffChange::Removed => "-",
  };
  let symbol = &record.record.symbol;
  println!(
    "{} {} {:<12} {:<32} {}:{}",
    sign,
    record.record.path,
    symbol.kind_name,
    symbol.name.as_deref().unwrap_or("<anonymous>"),
    symbol.range.start.line + 1,
    symbol.range.start.column + 1,
  );
}

fn print_related(query: &OutlineQuery, records: Vec<OutlineRecord>) -> Result<ExitCode> {
  match query.common().format {
    OutlineFormat::Text => {
      for record in &records {
        let symbol = &record.symbol;
        println!(
          "{:<20} {:<12} {:<32} {}:{} {}",
          symbol.reason.as_deref().unwrap_or("related"),
          symbol.kind_name,
          symbol.name.as_deref().unwrap_or("<anonymous>"),
          symbol.range.start.line + 1,
          symbol.range.start.column + 1,
          record.path,
        );
      }
    }
    OutlineFormat::Json => {
      println!("{}", serde_json::to_string_pretty(&records)?);
    }
    OutlineFormat::Jsonl => {
      for record in records {
        println!("{}", serde_json::to_string(&record)?);
      }
    }
  }
  Ok(ExitCode::SUCCESS)
}

fn enforce_limit(files: &mut Vec<OutlineFile>, limit: Option<usize>) {
  let Some(mut remaining) = limit else {
    return;
  };
  for file in files {
    limit_items(&mut file.items, &mut remaining);
  }
}

fn limit_items(items: &mut Vec<OutlineItem>, remaining: &mut usize) {
  let mut kept = vec![];
  for mut item in std::mem::take(items) {
    if *remaining == 0 {
      break;
    }
    *remaining -= 1;
    limit_items(&mut item.children, remaining);
    kept.push(item);
  }
  *items = kept;
}

fn print_text(files: &[OutlineFile]) {
  for file in files {
    println!("{}", file.path);
    print_text_items(&file.items, 1);
  }
}

fn print_text_items(items: &[OutlineItem], depth: usize) {
  for item in items {
    let indent = "  ".repeat(depth);
    let name = item.name.as_deref().unwrap_or("<anonymous>");
    let label = if item.exported {
      "export".to_string()
    } else {
      format!("{:?}", item.role).to_lowercase()
    };
    println!(
      "{indent}{:<12} {:<32} {}:{} {}",
      item.kind_name,
      name,
      item.range.start.line + 1,
      item.range.start.column + 1,
      label
    );
    print_text_items(&item.children, depth + 1);
  }
}

fn flatten_files(query: &OutlineQuery, files: &[OutlineFile]) -> Vec<OutlineRecord> {
  let mut records = vec![];
  for file in files {
    flatten_items_for_file(query.query_name(), file, &file.items, None, &mut records);
  }
  records
}

fn flatten_items_for_file(
  query: &'static str,
  file: &OutlineFile,
  items: &[OutlineItem],
  container: Option<OutlineContainer>,
  records: &mut Vec<OutlineRecord>,
) {
  for item in items {
    let current_container = Some(OutlineContainer {
      name: item.name.clone(),
      kind: item.kind,
      kind_name: item.kind_name,
      range: item.range.clone(),
    });
    records.push(OutlineRecord {
      path: file.path.clone(),
      language: file.language.clone(),
      query,
      symbol: flat_symbol(item, container.clone()),
    });
    flatten_items_for_file(query, file, &item.children, current_container, records);
  }
}

fn flat_symbol(item: &OutlineItem, container: Option<OutlineContainer>) -> OutlineFlatSymbol {
  OutlineFlatSymbol {
    name: item.name.clone(),
    kind: item.kind,
    kind_name: item.kind_name,
    role: item.role,
    range: item.range.clone(),
    selection_range: item.selection_range.clone(),
    signature: item.signature.clone(),
    exported: item.exported,
    node_kind: item.node_kind.clone(),
    container,
    score: None,
    reason: None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extracts_rust_symbols() {
    let src = "use std::path::Path;\npub struct RunArg {}\nfn run() {}\n";
    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let records = flatten_files(
      &OutlineQuery::Map(MapArg {
        common: test_common(),
        kind: vec![],
        depth: None,
      }),
      &[file],
    );
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("RunArg"))
    );
    assert!(records.iter().any(|r| r.symbol.role == SymbolRole::Import));
  }

  #[test]
  fn extracts_ts_members() {
    let src = r#"import { x } from "m"; export class Parser { parse() {} }"#;
    let file = extract_outline(
      PathBuf::from("test.ts"),
      SgLang::Builtin(SupportLang::TypeScript),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = OutlineQuery::Members(MembersArg {
      common: test_common(),
      of: "Parser".into(),
      of_kind: Some(SymbolKind::Class),
      kind: vec![SymbolKind::Method],
      recursive: false,
    });
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert_eq!(files[0].items.len(), 1);
    assert_eq!(files[0].items[0].name.as_deref(), Some("parse"));
    assert!(!files[0].items[0].exported);
  }

  #[test]
  fn trims_map_depth() {
    let src = "enum Commands { Run(RunArg) }\nstruct RunArg;\n";
    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = OutlineQuery::Map(MapArg {
      common: test_common(),
      kind: vec![],
      depth: Some(1),
    });
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(files[0].items.iter().all(|item| item.children.is_empty()));
  }

  #[test]
  fn extracts_from_custom_outline_rule() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let rule_path = dir.path().join("outline.yml");
    std::fs::write(
      &rule_path,
      r#"
extractors:
  - id: rust-function-only
    language: Rust
    kind: function
    role: definition
    name: field:name
    exported: never
    rule: { kind: function_item }
"#,
    )
    .expect("write outline rule");
    let mut common = test_common();
    common.no_default_outline_rules = true;
    common.outline_rules = vec![rule_path];
    let src = "pub struct RunArg {}\nfn custom() {}\n";
    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &common,
      &load_outline_catalog(&common).expect("load outline catalog"),
    )
    .expect("extract outline");
    let records = flatten_files(
      &OutlineQuery::Map(MapArg {
        common,
        kind: vec![],
        depth: None,
      }),
      &[file],
    );
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].symbol.name.as_deref(), Some("custom"));
    assert_eq!(records[0].symbol.kind, SymbolKind::Function.number());
  }

  fn test_common() -> OutlineCommonArg {
    OutlineCommonArg {
      lang: None,
      format: OutlineFormat::Json,
      budget: None,
      max_items: None,
      name: None,
      name_regex: None,
      flat: false,
      signature: true,
      outline_rules: vec![],
      no_default_outline_rules: false,
      input: InputArgs {
        no_ignore: vec![],
        stdin: false,
        follow: false,
        paths: vec![PathBuf::from(".")],
        globs: vec![],
        threads: 0,
      },
    }
  }

  fn test_catalog() -> OutlineCatalog {
    load_outline_catalog(&test_common()).expect("load outline catalog")
  }
}
