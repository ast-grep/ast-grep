use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
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
const CHILD_DIGEST_GROUP_LIMIT: usize = 8;

#[derive(Args)]
pub struct OutlineArg {
  #[clap(subcommand)]
  query: OutlineQuery,
}

#[derive(Subcommand, Clone)]
enum OutlineQuery {
  /// Return a compact structural map of files.
  Map(MapArg),
  /// Return import/dependency edges.
  Imports(ImportsArg),
  /// Return public/exported API symbols.
  Exports(ExportsArg),
  /// Return children of a container symbol.
  Members(MembersArg),
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
  #[serde(skip)]
  source_line: String,
  #[serde(skip)]
  child_digest: String,
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

impl OutlineCatalog {
  fn supported_langs(&self) -> HashSet<SgLang> {
    self
      .extractors
      .iter()
      .map(|extractor| extractor.language)
      .collect()
  }

  fn supports(&self, lang: SgLang) -> bool {
    self
      .extractors
      .iter()
      .any(|extractor| extractor.language == lang)
  }
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
  FirstNameLike,
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
  TextPrefixAny(Vec<String>),
  NotTextPrefixAny(Vec<String>),
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
    rule:
      all:
        - kind: function_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-class
    language: TypeScript
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: class_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
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
  - id: ts-method-signature
    language: TypeScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-property-signature
    language: TypeScript
    kind: field
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: property_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-method
    language: TypeScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-field
    language: TypeScript
    kind: field
    role: definition
    name: auto
    exported: never
    rule:
      all:
        - kind: public_field_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
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
        - not:
            inside:
              kind: statement_block
              stopBy: end
        - not:
            has:
              kind: arrow_function
              stopBy: end
  - id: ts-const-function
    language: TypeScript
    kind: function
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - has:
            kind: arrow_function
            stopBy: end
        - not:
            inside:
              kind: statement_block
              stopBy: end

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
    rule:
      all:
        - kind: function_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-class
    language: Tsx
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: class_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
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
  - id: tsx-method-signature
    language: Tsx
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-property-signature
    language: Tsx
    kind: field
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: property_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-method
    language: Tsx
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-field
    language: Tsx
    kind: field
    role: definition
    name: auto
    exported: never
    rule:
      all:
        - kind: public_field_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
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
        - not:
            inside:
              kind: statement_block
              stopBy: end
        - not:
            has:
              kind: arrow_function
              stopBy: end
  - id: tsx-const-function
    language: Tsx
    kind: function
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - has:
            kind: arrow_function
            stopBy: end
        - not:
            inside:
              kind: statement_block
              stopBy: end

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
    rule:
      all:
        - kind: function_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: js-class
    language: JavaScript
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: class_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: js-method
    language: JavaScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: js-field
    language: JavaScript
    kind: field
    role: definition
    name: auto
    exported: never
    rule:
      all:
        - kind: public_field_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
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
        - not:
            inside:
              kind: statement_block
              stopBy: end
        - not:
            has:
              kind: arrow_function
              stopBy: end
  - id: js-const-function
    language: JavaScript
    kind: function
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - has:
            kind: arrow_function
            stopBy: end
        - not:
            inside:
              kind: statement_block
              stopBy: end

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

  - id: java-package
    language: Java
    kind: package
    role: definition
    name: text
    exported: never
    rule: { kind: package_declaration }
  - id: java-import
    language: Java
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_declaration }
  - id: java-class
    language: Java
    kind: class
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: class_declaration }
  - id: java-record
    language: Java
    kind: struct
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: record_declaration }
  - id: java-interface
    language: Java
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: interface_declaration }
  - id: java-annotation
    language: Java
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: annotation_type_declaration }
  - id: java-enum
    language: Java
    kind: enum
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: enum_declaration }
  - id: java-method
    language: Java
    kind: method
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: method_declaration }
  - id: java-constructor
    language: Java
    kind: constructor
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: constructor_declaration }
  - id: java-public-static-final-constant
    language: Java
    kind: constant
    role: definition
    name: $NAME
    exported: always
    rule:
      pattern:
        context: class A { public static final $T $NAME = $V; }
        selector: field_declaration

  - id: kotlin-import
    language: Kotlin
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_header }
  - id: kotlin-class
    language: Kotlin
    kind: class
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?class\b'
  - id: kotlin-interface
    language: Kotlin
    kind: interface
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?interface\b'
  - id: kotlin-object
    language: Kotlin
    kind: object
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule: { kind: object_declaration }
  - id: kotlin-function
    language: Kotlin
    kind: function
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: function_declaration
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: kotlin-property
    language: Kotlin
    kind: variable
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: property_declaration
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: kotlin-typealias
    language: Kotlin
    kind: interface
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule: { kind: type_alias }

  - id: swift-import
    language: Swift
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_declaration }
  - id: swift-class
    language: Swift
    kind: class
    role: definition
    name: firstNameLike
    exported: textPrefixAny:public,open
    rule:
      any:
        - all:
            - kind: class_declaration
            - regex: '^\s*(public\s+|open\s+)?class\b'
        - all:
            - kind: function_declaration
            - regex: '^\s*(public\s+|open\s+)?class\b'
  - id: swift-struct
    language: Swift
    kind: struct
    role: definition
    name: firstNameLike
    exported: textPrefix:public
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?struct\b'
  - id: swift-enum
    language: Swift
    kind: enum
    role: definition
    name: firstNameLike
    exported: textPrefix:public
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?enum\b'
  - id: swift-protocol
    language: Swift
    kind: interface
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule: { kind: protocol_declaration }
  - id: swift-function
    language: Swift
    kind: function
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule:
      all:
        - kind: function_declaration
        - regex: '^\s*(@[A-Za-z_][A-Za-z0-9_]*(\([^)]*\))?\s+)*(public\s+|open\s+|internal\s+|fileprivate\s+|private\s+)?(static\s+|class\s+|mutating\s+|nonmutating\s+|override\s+|final\s+)*func\b'
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: swift-property
    language: Swift
    kind: variable
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule:
      all:
        - kind: property_declaration
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: swift-typealias
    language: Swift
    kind: interface
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule: { kind: typealias_declaration }
"#;

pub fn run_outline(arg: OutlineArg) -> Result<ExitCode> {
  let query = arg.query;
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
      OutlineQuery::Imports(arg) => &arg.common,
      OutlineQuery::Exports(arg) => &arg.common,
      OutlineQuery::Members(arg) => &arg.common,
    }
  }

  fn query_name(&self) -> &'static str {
    match self {
      OutlineQuery::Map(_) => "map",
      OutlineQuery::Imports(_) => "imports",
      OutlineQuery::Exports(_) => "exports",
      OutlineQuery::Members(_) => "members",
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
  let common = Arc::new(common.clone());
  let catalog = Arc::new(load_outline_catalog(&common)?);
  let supported_langs = catalog.supported_langs();
  if let Some(lang) = common.lang {
    if !supported_langs.contains(&lang) {
      return Ok(vec![]);
    }
  } else if supported_langs.is_empty() {
    return Ok(vec![]);
  }
  let walker = build_walk(&common, supported_langs)?;
  let (tx, rx) = mpsc::channel();
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
        if !catalog.supports(lang) {
          return WalkState::Continue;
        }
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

fn build_walk(common: &OutlineCommonArg, supported_langs: HashSet<SgLang>) -> Result<WalkParallel> {
  if let Some(lang) = common.lang {
    common.input.walk_lang(lang)
  } else {
    common.input.walk_langs(supported_langs.into_iter())
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
  let mut items = nest_items(items);
  if lang == SgLang::Builtin(SupportLang::Go) {
    attach_go_receiver_methods(&mut items);
  }
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
  } else if source == "firstNameLike" {
    NameSource::FirstNameLike
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
      } else if let Some(prefixes) = policy.strip_prefix("textPrefixAny:") {
        ExportPolicy::TextPrefixAny(parse_prefixes(prefixes))
      } else if let Some(prefixes) = policy.strip_prefix("notTextPrefixAny:") {
        ExportPolicy::NotTextPrefixAny(parse_prefixes(prefixes))
      } else if let Some(kind) = policy.strip_prefix("ancestorKind:") {
        ExportPolicy::AncestorKind(kind.into())
      } else {
        ExportPolicy::Auto
      }
    }
  }
}

fn parse_prefixes(prefixes: &str) -> Vec<String> {
  prefixes
    .split(',')
    .map(str::trim)
    .filter(|prefix| !prefix.is_empty())
    .map(str::to_string)
    .collect()
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
    source_line: signature(node),
    child_digest: String::new(),
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
    NameSource::FirstNameLike => {
      return resolve_first_name_like(node);
    }
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
  if let resolved @ (Some(_), Some(_)) = resolve_first_name_like(node) {
    return resolved;
  }
  (None, None)
}

fn resolve_first_name_like(node: &SgNode<'_>) -> (Option<String>, Option<OutlineRange>) {
  if let Some(name) = node
    .dfs()
    .find(|name| is_name_like_node(name) && !is_modifier_metadata(name))
  {
    (
      Some(name.text().trim().to_string()),
      Some(node_range(&name)),
    )
  } else {
    (None, None)
  }
}

fn is_name_like_node(node: &SgNode<'_>) -> bool {
  matches!(
    node.kind().as_ref(),
    "identifier"
      | "type_identifier"
      | "field_identifier"
      | "property_identifier"
      | "shorthand_property_identifier"
      | "simple_identifier"
      | "constant"
  )
}

fn is_modifier_metadata(node: &SgNode<'_>) -> bool {
  node.ancestors().any(|ancestor| {
    matches!(
      ancestor.kind().as_ref(),
      "modifiers" | "annotation" | "marker_annotation" | "annotation_argument_list"
    )
  })
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
      .trim_start_matches("import ")
      .trim_start_matches("package ")
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
    ExportPolicy::TextPrefixAny(prefixes) => {
      let text = node.text();
      let text = text.trim_start();
      prefixes.iter().any(|prefix| text.starts_with(prefix))
    }
    ExportPolicy::NotTextPrefixAny(prefixes) => {
      let text = node.text();
      let text = text.trim_start();
      !prefixes.iter().any(|prefix| text.starts_with(prefix))
    }
    ExportPolicy::AncestorKind(kind) => node.ancestors().any(|n| n.kind().as_ref() == kind),
    ExportPolicy::Auto => false,
  }
}

fn signature(node: &SgNode<'_>) -> String {
  let text = node.text();
  text
    .lines()
    .find(|line| !line.trim_start().starts_with('@'))
    .or_else(|| text.lines().next())
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

fn attach_go_receiver_methods(items: &mut Vec<OutlineItem>) {
  let mut roots = std::mem::take(items);
  let mut kept = Vec::with_capacity(roots.len());
  let mut methods = vec![];
  for item in roots.drain(..) {
    if item.kind == SymbolKind::Method.number()
      && let Some(receiver) = go_receiver_type(&item.source_line)
    {
      methods.push((receiver, item));
    } else {
      kept.push(item);
    }
  }
  for (receiver, method) in methods {
    if let Some(parent) = kept
      .iter_mut()
      .find(|item| item.name.as_deref() == Some(receiver.as_str()))
    {
      parent.children.push(method);
      parent
        .children
        .sort_by_key(|child| (child.range.start.byte, Reverse(child.range.end.byte)));
    } else {
      kept.push(method);
    }
  }
  kept.sort_by_key(|item| (item.range.start.byte, Reverse(item.range.end.byte)));
  *items = kept;
}

fn go_receiver_type(signature: &str) -> Option<String> {
  let signature = signature.trim_start();
  let rest = signature.strip_prefix("func ")?;
  let rest = rest.strip_prefix('(')?;
  let (receiver, _) = rest.split_once(')')?;
  receiver
    .split_whitespace()
    .last()
    .map(|ty| ty.trim_start_matches('*').to_string())
    .filter(|ty| !ty.is_empty())
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
        item.role != SymbolRole::Import
          && kind_matches(item, &arg.kind)
          && common_matches(item, common)
      });
      if let Some(depth) = arg.depth {
        trim_depth(&mut items, depth);
      } else {
        set_child_digests(&mut items);
        trim_depth(&mut items, 1);
      }
      items
    }
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

fn set_child_digests(items: &mut [OutlineItem]) {
  for item in items {
    item.child_digest = child_digest(&item.children);
    set_child_digests(&mut item.children);
  }
}

fn child_digest(children: &[OutlineItem]) -> String {
  let mut groups: Vec<(&'static str, Vec<String>)> = vec![];
  for child in children {
    let label = text_group_label(child);
    let name = child_digest_name(child);
    if name.is_empty() {
      continue;
    }
    if let Some((_, names)) = groups.iter_mut().find(|(group, _)| *group == label) {
      if !names.contains(&name) {
        names.push(name);
      }
    } else {
      groups.push((label, vec![name]));
    }
  }
  groups.sort_by_key(|(label, _)| child_digest_group_rank(label));
  groups
    .into_iter()
    .map(|(label, names)| format!("{label}: {}", capped_digest_names(&names)))
    .collect::<Vec<_>>()
    .join("; ")
}

fn capped_digest_names(names: &[String]) -> String {
  if names.len() <= CHILD_DIGEST_GROUP_LIMIT {
    return names.join(", ");
  }
  let shown = names[..CHILD_DIGEST_GROUP_LIMIT].join(", ");
  let hidden = names.len() - CHILD_DIGEST_GROUP_LIMIT;
  format!("{shown}, ... +{hidden} more")
}

fn child_digest_group_rank(label: &str) -> u8 {
  match label {
    "field" => 0,
    "property" => 1,
    "constant" => 2,
    "variable" => 3,
    "constructor" => 4,
    "method" => 5,
    "function" => 6,
    _ => text_group_rank(label),
  }
}

fn child_digest_name(item: &OutlineItem) -> String {
  item
    .name
    .clone()
    .unwrap_or_else(|| item.source_line.trim().to_string())
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

fn kind_matches(item: &OutlineItem, kinds: &[SymbolKind]) -> bool {
  kinds.is_empty() || kinds.iter().any(|kind| item.kind == kind.number())
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
    if file.items.is_empty() {
      println!("nothing found");
    } else {
      print_text_items(&file.items);
    }
  }
}

fn print_text_items(items: &[OutlineItem]) {
  let mut roots = items.iter().collect::<Vec<_>>();
  roots.sort_by_key(|item| {
    (
      text_group_rank(text_group_label(item)),
      item.range.start.line,
      item.range.start.column,
    )
  });
  let mut current_label = None;
  for item in roots {
    let label = text_group_label(item);
    if current_label != Some(label) {
      current_label = Some(label);
      println!("{label}:");
    }
    print_text_tree(item, 0);
  }
}

fn print_text_tree(item: &OutlineItem, depth: usize) {
  let indent = "  ".repeat(depth);
  let source = if item.source_line.is_empty() {
    item.name.as_deref().unwrap_or("<anonymous>")
  } else {
    item.source_line.as_str()
  };
  println!("{}: {indent}{}", item.range.start.line + 1, source,);
  if !item.child_digest.is_empty() {
    println!("{}  {}", "  ".repeat(depth), item.child_digest);
  }
  for child in &item.children {
    print_text_tree(child, depth + 1);
  }
}

fn text_group_label(item: &OutlineItem) -> &'static str {
  let source = item.source_line.trim_start();
  if starts_with_any(
    source,
    &[
      "type ",
      "pub type ",
      "export type ",
      "public typealias ",
      "typealias ",
    ],
  ) {
    return "type";
  }
  if starts_with_any(source, &["trait ", "pub trait "]) {
    return "trait";
  }
  if starts_with_any(source, &["protocol ", "public protocol ", "open protocol "]) {
    return "protocol";
  }
  if starts_with_any(source, &["impl ", "pub impl "]) {
    return "impl";
  }
  kind_text_label(item.kind)
}

fn starts_with_any(source: &str, prefixes: &[&str]) -> bool {
  prefixes.iter().any(|prefix| source.starts_with(prefix))
}

fn text_group_rank(label: &str) -> u8 {
  match label {
    "package" => 0,
    "module" => 1,
    "namespace" => 2,
    "class" => 3,
    "interface" => 4,
    "trait" => 5,
    "protocol" => 6,
    "struct" => 7,
    "enum" => 8,
    "type" => 9,
    "impl" => 10,
    "function" => 11,
    "method" => 12,
    "constructor" => 13,
    "field" => 14,
    "property" => 15,
    "constant" => 16,
    "variable" => 17,
    "enum member" => 18,
    "type parameter" => 19,
    _ => 20,
  }
}

fn kind_text_label(kind: u8) -> &'static str {
  match kind {
    k if k == SymbolKind::File.number() => "file",
    k if k == SymbolKind::Module.number() => "module",
    k if k == SymbolKind::Namespace.number() => "namespace",
    k if k == SymbolKind::Package.number() => "package",
    k if k == SymbolKind::Class.number() => "class",
    k if k == SymbolKind::Method.number() => "method",
    k if k == SymbolKind::Property.number() => "property",
    k if k == SymbolKind::Field.number() => "field",
    k if k == SymbolKind::Constructor.number() => "constructor",
    k if k == SymbolKind::Enum.number() => "enum",
    k if k == SymbolKind::Interface.number() => "interface",
    k if k == SymbolKind::Function.number() => "function",
    k if k == SymbolKind::Variable.number() => "variable",
    k if k == SymbolKind::Constant.number() => "constant",
    k if k == SymbolKind::String.number() => "string",
    k if k == SymbolKind::Number.number() => "number",
    k if k == SymbolKind::Boolean.number() => "boolean",
    k if k == SymbolKind::Array.number() => "array",
    k if k == SymbolKind::Object.number() => "object",
    k if k == SymbolKind::Key.number() => "key",
    k if k == SymbolKind::Null.number() => "null",
    k if k == SymbolKind::EnumMember.number() => "enum member",
    k if k == SymbolKind::Struct.number() => "struct",
    k if k == SymbolKind::Event.number() => "event",
    k if k == SymbolKind::Operator.number() => "operator",
    k if k == SymbolKind::TypeParameter.number() => "type parameter",
    _ => "symbol",
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
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("RunArg"))
    );
    assert!(records.iter().all(|r| r.symbol.role != SymbolRole::Import));
    assert!(files[0].items.iter().any(|item| {
      item.name.as_deref() == Some("RunArg") && item.source_line == "pub struct RunArg {}"
    }));

    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = imports_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
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
  fn ts_map_skips_local_variables() {
    let src = r#"
export interface MockLlmServer {
  readonly url: string;
  requestCount(): number;
}

const exportedShape = 1;
let logsCounter = 1;

export function retry() {
  const result = Promise.resolve();
  const localTyped: { parentPort: string } = { parentPort: "x" };
  let attempt = 0;
  return result;
}
"#;
    let file = extract_outline(
      PathBuf::from("test.ts"),
      SgLang::Builtin(SupportLang::TypeScript),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("MockLlmServer"))
    );
    let mock = files[0]
      .items
      .iter()
      .find(|item| item.name.as_deref() == Some("MockLlmServer"))
      .expect("mock interface");
    assert!(mock.children.is_empty());
    assert_eq!(mock.child_digest, "field: url; method: requestCount");
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("exportedShape"))
    );
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("retry"))
    );
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("result"))
    );
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("attempt"))
    );
    let retry = files[0]
      .items
      .iter()
      .find(|item| item.name.as_deref() == Some("retry"))
      .expect("retry function");
    assert!(retry.child_digest.is_empty());
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("logsCounter"))
    );
  }

  #[test]
  fn extracts_java_symbols() {
    let src = r#"
package demo;
import java.util.List;
public class Foo {
  public static final int SIZE = 1;
  public Foo() {}
  public void bar() {}
}
public record Rec(int id) {}
"#;
    let file = extract_outline(
      PathBuf::from("test.java"),
      SgLang::Builtin(SupportLang::Java),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("java.util.List"))
    );
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Foo") && r.symbol.kind == SymbolKind::Class.number()
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Rec") && r.symbol.kind == SymbolKind::Struct.number()
    }));
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("SIZE"))
    );
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("bar"))
    );
    let query = OutlineQuery::Members(MembersArg {
      common: test_common(),
      of: "Foo".into(),
      of_kind: Some(SymbolKind::Class),
      kind: vec![],
      recursive: false,
    });
    let file = extract_outline(
      PathBuf::from("test.java"),
      SgLang::Builtin(SupportLang::Java),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("SIZE") && r.symbol.kind == SymbolKind::Constant.number()
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("bar") && r.symbol.kind == SymbolKind::Method.number()
    }));
    let query = OutlineQuery::Exports(ExportsArg {
      common: test_common(),
      definitions_only: true,
    });
    let file = extract_outline(
      PathBuf::from("test.java"),
      SgLang::Builtin(SupportLang::Java),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("Foo"))
    );
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("Rec"))
    );
  }

  #[test]
  fn extracts_kotlin_symbols() {
    let src = r#"
import a.b.C
class Foo {
  val name: String = ""
  fun bar() {}
}
private class Hidden
typealias Alias = String
object Obj {}
interface I {}
"#;
    let file = extract_outline(
      PathBuf::from("test.kt"),
      SgLang::Builtin(SupportLang::Kotlin),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = OutlineQuery::Members(MembersArg {
      common: test_common(),
      of: "Foo".into(),
      of_kind: Some(SymbolKind::Class),
      kind: vec![],
      recursive: false,
    });
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("name"))
    );
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("bar"))
    );
    let query = OutlineQuery::Exports(ExportsArg {
      common: test_common(),
      definitions_only: true,
    });
    let file = extract_outline(
      PathBuf::from("test.kt"),
      SgLang::Builtin(SupportLang::Kotlin),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("Foo"))
    );
    assert!(
      !files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("Hidden"))
    );
  }

  #[test]
  fn extracts_swift_symbols() {
    let src = r#"
import Foundation
public class Foo {
  public let name: String = ""
  public func bar() {}
}
public struct Box {}
public enum Mode { case on }
public protocol P {}
public typealias Alias = String
"#;
    let file = extract_outline(
      PathBuf::from("test.swift"),
      SgLang::Builtin(SupportLang::Swift),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("Foundation"))
    );
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Foo") && r.symbol.kind == SymbolKind::Class.number()
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Box") && r.symbol.kind == SymbolKind::Struct.number()
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Mode") && r.symbol.kind == SymbolKind::Enum.number()
    }));
    let query = OutlineQuery::Members(MembersArg {
      common: test_common(),
      of: "Foo".into(),
      of_kind: Some(SymbolKind::Class),
      kind: vec![],
      recursive: false,
    });
    let file = extract_outline(
      PathBuf::from("test.swift"),
      SgLang::Builtin(SupportLang::Swift),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("name"))
    );
    assert!(
      files[0]
        .items
        .iter()
        .any(|item| item.name.as_deref() == Some("bar"))
    );
  }

  #[test]
  fn extracts_swift_open_class_with_spi_method() {
    let src = r#"
import Foundation
open class Session: @unchecked Sendable {
  public static let `default` = Session()
  @_spi(WebSocket) open func webSocketRequest() {}
}
"#;
    let file = extract_outline(
      PathBuf::from("test.swift"),
      SgLang::Builtin(SupportLang::Swift),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Session") && r.symbol.kind == SymbolKind::Class.number()
    }));
    assert!(!records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("webSocketRequest")
        && r.symbol.kind == SymbolKind::Function.number()
        && r.symbol.range.start.line == 2
    }));
  }

  #[test]
  fn go_receiver_methods_are_members() {
    let src = r#"
type RouterGroup struct {}
func (group *RouterGroup) Use() {}
func (group RouterGroup) BasePath() string { return "" }
func (group *RouterGroup) Group() {}
func (group *RouterGroup) Handle() {}
func (group *RouterGroup) POST() {}
func (group *RouterGroup) GET() {}
func (group *RouterGroup) DELETE() {}
func (group *RouterGroup) PATCH() {}
func (group *RouterGroup) PUT() {}
func (group *RouterGroup) OPTIONS() {}
func standalone() {}
"#;
    let file = extract_outline(
      PathBuf::from("test.go"),
      SgLang::Builtin(SupportLang::Go),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("RouterGroup"))
    );
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("standalone"))
    );
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("Use"))
    );
    let router_group = files[0]
      .items
      .iter()
      .find(|item| item.name.as_deref() == Some("RouterGroup"))
      .expect("router group");
    assert_eq!(
      router_group.child_digest,
      "method: Use, BasePath, Group, Handle, POST, GET, DELETE, PATCH, ... +2 more"
    );

    let file = extract_outline(
      PathBuf::from("test.go"),
      SgLang::Builtin(SupportLang::Go),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = OutlineQuery::Map(MapArg {
      common: test_common(),
      kind: vec![],
      depth: Some(2),
    });
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let records = flatten_files(&query, &files);
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("Use"))
    );

    let file = extract_outline(
      PathBuf::from("test.go"),
      SgLang::Builtin(SupportLang::Go),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = OutlineQuery::Members(MembersArg {
      common: test_common(),
      of: "RouterGroup".into(),
      of_kind: None,
      kind: vec![SymbolKind::Method],
      recursive: false,
    });
    let mut files = vec![file];
    apply_query(&query, &mut files);
    let names = files[0]
      .items
      .iter()
      .filter_map(|item| item.name.as_deref())
      .collect::<Vec<_>>();
    assert_eq!(
      names,
      vec![
        "Use", "BasePath", "Group", "Handle", "POST", "GET", "DELETE", "PATCH", "PUT", "OPTIONS"
      ]
    );
  }

  #[test]
  fn map_defaults_to_top_level() {
    let src = "enum Commands { Run(RunArg) }\nstruct RunArg;\n";
    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query();
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(files[0].items.iter().all(|item| item.children.is_empty()));
  }

  #[test]
  fn map_depth_includes_nested_items() {
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
      depth: Some(2),
    });
    let mut files = vec![file];
    apply_query(&query, &mut files);
    assert!(files[0].items.iter().any(|item| !item.children.is_empty()));
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

  #[test]
  fn catalog_reports_only_languages_with_outline_rules() {
    let catalog = test_catalog();
    assert!(catalog.supports(SgLang::Builtin(SupportLang::Rust)));
    assert!(catalog.supports(SgLang::Builtin(SupportLang::TypeScript)));
    assert!(!catalog.supports(SgLang::Builtin(SupportLang::Html)));

    let empty = OutlineCatalog { extractors: vec![] };
    assert!(empty.supported_langs().is_empty());
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

  fn map_query() -> OutlineQuery {
    OutlineQuery::Map(MapArg {
      common: test_common(),
      kind: vec![],
      depth: None,
    })
  }

  fn imports_query() -> OutlineQuery {
    OutlineQuery::Imports(ImportsArg {
      common: test_common(),
      to: None,
      bindings: false,
    })
  }
}
