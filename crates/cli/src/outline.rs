use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::mpsc;

use anyhow::{Result, anyhow};
use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore, from_str};
use ast_grep_core::Node;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_language::{Language, LanguageExt, SupportLang};
use clap::{Args, ValueEnum};
use ignore::{DirEntry, WalkParallel, WalkState};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::lang::SgLang;
use crate::print::JsonStyle;
use crate::utils::{InputArgs, read_file};

type SgDoc = StrDoc<SgLang>;
type SgNode<'a> = Node<'a, SgDoc>;
const CHILD_DIGEST_GROUP_LIMIT: usize = 8;

#[derive(Args, Clone)]
pub struct OutlineArg {
  #[clap(flatten)]
  common: OutlineCommonArg,
  /// Filter outline symbols by SymbolType name.
  #[clap(long = "type", value_name = "TYPE", value_delimiter = ',', action = clap::ArgAction::Append)]
  symbol_type: Vec<SymbolType>,
  /// Select records by role facet. Repeatable. Comma-separated roles are ANDed.
  #[clap(long, value_name = "ROLE", action = clap::ArgAction::Append)]
  role: Vec<RoleFilter>,
  /// Control whether structural members are shown.
  #[clap(long, default_value = "names", value_name = "MODE")]
  members: OutlineMembers,
}

#[derive(Args, Clone)]
struct OutlineCommonArg {
  /// Language to parse input as. If absent, infer from file path.
  #[clap(short, long)]
  lang: Option<SgLang>,
  /// Output outline in structured JSON.
  #[clap(
      long,
      value_name = "STYLE",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pretty"
  )]
  json: Option<JsonStyle>,
  /// Regex pattern over role-relevant fields.
  #[clap(long = "match", value_name = "REGEX", action = clap::ArgAction::Append)]
  matches: Vec<Regex>,
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

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[value(rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
enum SymbolType {
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

impl SymbolType {
  fn label(self) -> &'static str {
    match self {
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
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[value(rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
enum SymbolRole {
  Definition,
  Import,
  Export,
}

impl SymbolRole {
  fn parse(value: &str) -> Option<Self> {
    match value {
      "definition" | "definitions" => Some(Self::Definition),
      "import" | "imports" => Some(Self::Import),
      "export" | "exports" => Some(Self::Export),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RoleFilter {
  any: bool,
  roles: Vec<SymbolRole>,
}

impl FromStr for RoleFilter {
  type Err = String;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    if value == "any" {
      return Ok(Self {
        any: true,
        roles: vec![],
      });
    }
    let roles = value
      .split(',')
      .map(str::trim)
      .filter(|role| !role.is_empty())
      .map(|role| SymbolRole::parse(role).ok_or_else(|| format!("invalid role: {role}")))
      .collect::<Result<Vec<_>, _>>()?;
    if roles.is_empty() {
      Err("role filter cannot be empty".into())
    } else {
      Ok(Self { any: false, roles })
    }
  }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[value(rename_all = "camelCase")]
enum OutlineMembers {
  None,
  Names,
  Lines,
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
  symbol_type: SymbolType,
  roles: Vec<SymbolRole>,
  range: OutlineRange,
  #[serde(skip_serializing_if = "Option::is_none")]
  signature: Option<String>,
  #[serde(skip)]
  source_line: String,
  #[serde(rename = "memberDigest", skip_serializing_if = "String::is_empty")]
  child_digest: String,
  ast_kind: String,
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
  symbol_type: SymbolType,
  range: OutlineRange,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineRecord {
  path: String,
  language: String,
  symbol: OutlineFlatSymbol,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineFlatSymbol {
  name: Option<String>,
  symbol_type: SymbolType,
  roles: Vec<SymbolRole>,
  range: OutlineRange,
  #[serde(skip_serializing_if = "Option::is_none")]
  signature: Option<String>,
  #[serde(rename = "memberDigest", skip_serializing_if = "String::is_empty")]
  child_digest: String,
  ast_kind: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  container: Option<OutlineContainer>,
}

struct RuleSpec {
  symbol_type: SymbolType,
  roles: Vec<SymbolRole>,
  add_roles: Vec<(SymbolRole, ExportPolicy)>,
  name: NameSource,
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
  #[serde(rename = "kind")]
  symbol_type: SymbolType,
  #[serde(default)]
  role: Option<SymbolRole>,
  #[serde(default)]
  roles: Vec<SymbolRole>,
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  exported: Option<String>,
  #[serde(default, rename = "addRoles")]
  add_roles: HashMap<String, String>,
  #[serde(default)]
  target: Option<String>,
  #[serde(default)]
  alias: Option<String>,
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
    exported: textPrefix:pub
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
  let common = &arg.common;
  let mut files = if common.input.stdin {
    vec![outline_stdin(&arg)?]
  } else {
    outline_paths(&arg)?
  };
  apply_view(&arg, &mut files);
  print_outline(&arg, files)
}

fn outline_stdin(arg: &OutlineArg) -> Result<OutlineFile> {
  let common = &arg.common;
  let catalog = load_outline_catalog(common)?;
  let lang = common
    .lang
    .ok_or_else(|| anyhow!("--stdin requires --lang"))?;
  let src = std::io::read_to_string(std::io::stdin())?;
  extract_outline("STDIN".into(), lang, &src, common, &catalog)
}

fn outline_paths(arg: &OutlineArg) -> Result<Vec<OutlineFile>> {
  let common = &arg.common;
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
  _common: &OutlineCommonArg,
  catalog: &OutlineCatalog,
) -> Result<OutlineFile> {
  let grep = lang.ast_grep(src);
  let root = grep.root();
  let mut items = vec![];
  for spec in outline_rules(lang, catalog)? {
    for matched in root.find_all(&spec.matcher) {
      if let Some(item) = make_item(&matched, lang, &spec, true) {
        items.push(item);
      }
    }
  }
  dedup_items(&mut items);
  items.sort_by_key(|i| (i.range.start.byte, Reverse(i.range.end.byte)));
  let mut items = nest_items(items);
  prune_outline_members(lang, &mut items);
  if lang == SgLang::Builtin(SupportLang::Go) {
    attach_go_receiver_methods(&mut items);
    prune_outline_members(lang, &mut items);
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
      let _target = &spec.target;
      let _alias = &spec.alias;
      let matcher = spec.core.get_matcher(env.clone())?;
      let roles = compile_base_roles(&spec)?;
      let mut add_roles = compile_add_roles(spec.add_roles)?;
      if let Some(exported) = spec.exported {
        add_roles.push((SymbolRole::Export, parse_export_policy(Some(exported))));
      }
      Ok(RuleSpec {
        symbol_type: spec.symbol_type,
        roles,
        add_roles,
        name: parse_name_source(spec.name),
        matcher,
      })
    })
    .collect()
}

fn compile_base_roles(spec: &SerializableOutlineExtractor) -> Result<Vec<SymbolRole>> {
  let mut roles = vec![];
  if let Some(role) = spec.role {
    add_role(&mut roles, role);
  }
  for role in &spec.roles {
    add_role(&mut roles, *role);
  }
  if roles.is_empty() {
    return Err(anyhow!("outline extractor {} must define roles", spec.id));
  }
  Ok(roles)
}

fn compile_add_roles(
  add_roles: HashMap<String, String>,
) -> Result<Vec<(SymbolRole, ExportPolicy)>> {
  add_roles
    .into_iter()
    .map(|(role, policy)| {
      let role = SymbolRole::parse(&role).ok_or_else(|| anyhow!("invalid outline role: {role}"))?;
      Ok((role, parse_export_policy(Some(policy))))
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
  let name = resolve_name(matched, lang, spec);
  if spec.roles.contains(&SymbolRole::Definition) && name.is_none() {
    return None;
  }
  let roles = item_roles(&spec.roles, &spec.add_roles, node, name.as_deref());
  let symbol_type = spec.symbol_type;
  Some(OutlineItem {
    name,
    symbol_type,
    roles,
    range: node_range(node),
    signature: include_signature.then(|| signature(node)),
    source_line: signature(node),
    child_digest: String::new(),
    ast_kind: node.kind().to_string(),
    children: vec![],
  })
}

fn item_roles(
  base: &[SymbolRole],
  add_roles: &[(SymbolRole, ExportPolicy)],
  node: &SgNode<'_>,
  name: Option<&str>,
) -> Vec<SymbolRole> {
  let mut roles = vec![];
  for role in base {
    add_role(&mut roles, *role);
  }
  for (role, policy) in add_roles {
    if role_predicate_matches(node, policy, name) {
      add_role(&mut roles, *role);
    }
  }
  let source = node.text();
  let source = source.trim_start();
  if roles.contains(&SymbolRole::Export) && is_forwarded_export(source) {
    add_role(&mut roles, SymbolRole::Import);
  }
  roles
}

fn add_role(roles: &mut Vec<SymbolRole>, role: SymbolRole) {
  if !roles.contains(&role) {
    roles.push(role);
  }
}

fn is_forwarded_export(source: &str) -> bool {
  source.starts_with("pub use ")
    || source.contains(" from ")
    || source.contains(" from\"")
    || source.contains(" from'")
}

fn resolve_name(
  matched: &ast_grep_core::NodeMatch<SgDoc>,
  lang: SgLang,
  spec: &RuleSpec,
) -> Option<String> {
  let node = matched.get_node();
  match &spec.name {
    NameSource::Text => return Some(import_export_name(node)),
    NameSource::FirstNameLike => {
      return resolve_first_name_like(node);
    }
    NameSource::Field(field) => {
      if let Some(name) = node.field(field) {
        return Some(name.text().trim().to_string());
      }
    }
    NameSource::MetaVar(var) => {
      if let Some(name) = matched.get_env().get_match(var) {
        return Some(name.text().trim().to_string());
      }
    }
    NameSource::Auto => {}
  }
  if !spec.roles.contains(&SymbolRole::Definition)
    && spec
      .roles
      .iter()
      .any(|role| matches!(role, SymbolRole::Import | SymbolRole::Export))
  {
    return Some(import_export_name(node));
  }
  if let Some(name) = node.field("name") {
    return Some(name.text().trim().to_string());
  }
  if node.kind().as_ref() == "lexical_declaration" || node.kind().as_ref() == "variable_declaration"
  {
    if let Some(name) = node.dfs().find(|n| {
      matches!(
        n.kind().as_ref(),
        "identifier" | "shorthand_property_identifier_pattern"
      )
    }) {
      return Some(name.text().trim().to_string());
    }
  }
  if lang == SgLang::Builtin(SupportLang::Go)
    && let Some(name) = node.dfs().find(|n| n.kind().as_ref() == "identifier")
  {
    return Some(name.text().trim().to_string());
  }
  if node.kind().as_ref() == "impl_item" {
    let text = node.text();
    let name = text
      .trim_start()
      .strip_prefix("impl")
      .map(str::trim)
      .and_then(|s| s.split([' ', '{', '<']).find(|s| !s.is_empty()))
      .map(str::to_string);
    return name;
  }
  if let resolved @ Some(_) = resolve_first_name_like(node) {
    return resolved;
  }
  None
}

fn resolve_first_name_like(node: &SgNode<'_>) -> Option<String> {
  node
    .dfs()
    .find(|name| is_name_like_node(name) && !is_modifier_metadata(name))
    .map(|name| name.text().trim().to_string())
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

fn role_predicate_matches(node: &SgNode<'_>, policy: &ExportPolicy, name: Option<&str>) -> bool {
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
      i.symbol_type,
      i.name.clone(),
    )
  });
  let mut deduped: Vec<OutlineItem> = vec![];
  for item in std::mem::take(items) {
    if let Some(existing) = deduped.last_mut()
      && existing.range.start.byte == item.range.start.byte
      && existing.range.end.byte == item.range.end.byte
      && existing.symbol_type == item.symbol_type
      && existing.name == item.name
    {
      for role in item.roles {
        if !existing.roles.contains(&role) {
          existing.roles.push(role);
        }
      }
    } else {
      deduped.push(item);
    }
  }
  *items = deduped;
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
    if item.symbol_type == SymbolType::Method
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

fn prune_outline_members(lang: SgLang, items: &mut [OutlineItem]) {
  for parent in items {
    let mut kept = vec![];
    for mut child in std::mem::take(&mut parent.children) {
      prune_outline_members(lang, std::slice::from_mut(&mut child));
      if is_outline_member(lang, parent, &child) {
        kept.push(child);
      }
    }
    parent.children = kept;
  }
}

fn is_outline_member(lang: SgLang, parent: &OutlineItem, child: &OutlineItem) -> bool {
  if is_function_like(parent) && is_js_family(lang) && child.symbol_type == SymbolType::Function {
    return true;
  }
  is_container_type(parent.symbol_type) && is_member_type(child.symbol_type)
}

fn is_js_family(lang: SgLang) -> bool {
  matches!(
    lang,
    SgLang::Builtin(SupportLang::JavaScript)
      | SgLang::Builtin(SupportLang::TypeScript)
      | SgLang::Builtin(SupportLang::Tsx)
  )
}

fn is_function_like(item: &OutlineItem) -> bool {
  matches!(
    item.symbol_type,
    SymbolType::Function | SymbolType::Method | SymbolType::Constructor
  )
}

fn is_container_type(symbol_type: SymbolType) -> bool {
  matches!(
    symbol_type,
    SymbolType::Class
      | SymbolType::Struct
      | SymbolType::Interface
      | SymbolType::Enum
      | SymbolType::Object
      | SymbolType::Namespace
      | SymbolType::Module
      | SymbolType::Package
  )
}

fn is_member_type(symbol_type: SymbolType) -> bool {
  matches!(
    symbol_type,
    SymbolType::Class
      | SymbolType::Struct
      | SymbolType::Interface
      | SymbolType::Enum
      | SymbolType::Object
      | SymbolType::Namespace
      | SymbolType::Module
      | SymbolType::Function
      | SymbolType::Method
      | SymbolType::Constructor
      | SymbolType::Property
      | SymbolType::Field
      | SymbolType::Constant
      | SymbolType::Variable
      | SymbolType::EnumMember
      | SymbolType::TypeParameter
  )
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

fn apply_view(arg: &OutlineArg, files: &mut Vec<OutlineFile>) {
  let common = &arg.common;
  for file in files.iter_mut() {
    file.items = filter_items(std::mem::take(&mut file.items), arg, common);
  }
  let keep_empty_files = arg.is_default_map_view() || is_direct_file_input(common);
  files.retain(|file| !file.items.is_empty() || keep_empty_files);
}

fn is_direct_file_input(common: &OutlineCommonArg) -> bool {
  common.input.stdin || common.input.paths.iter().any(|path| path.is_file())
}

fn filter_items(
  items: Vec<OutlineItem>,
  arg: &OutlineArg,
  common: &OutlineCommonArg,
) -> Vec<OutlineItem> {
  let mut items = if has_anchor_filters(arg, common) {
    collect_matching_anchors(items, arg, common)
  } else {
    items
      .into_iter()
      .filter(|item| role_matches(item, &arg.role))
      .collect()
  };
  match arg.members {
    OutlineMembers::None => trim_depth(&mut items, 1),
    OutlineMembers::Names => {
      set_child_digests(&mut items);
      trim_depth(&mut items, 1);
    }
    OutlineMembers::Lines => trim_depth(&mut items, 2),
  }
  items
}

impl OutlineArg {
  fn is_default_map_view(&self) -> bool {
    self.role.is_empty() && !has_anchor_filters(self, &self.common)
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

fn collect_matching_anchors(
  items: Vec<OutlineItem>,
  arg: &OutlineArg,
  common: &OutlineCommonArg,
) -> Vec<OutlineItem> {
  let mut ret = vec![];
  for item in items {
    collect_matching_anchor(item, arg, common, &mut ret);
  }
  ret
}

fn collect_matching_anchor(
  mut item: OutlineItem,
  arg: &OutlineArg,
  common: &OutlineCommonArg,
  ret: &mut Vec<OutlineItem>,
) {
  if item_matches(&item, arg, common) {
    ret.push(item);
  } else {
    for child in std::mem::take(&mut item.children) {
      collect_matching_anchor(child, arg, common, ret);
    }
  }
}

fn type_matches(item: &OutlineItem, types: &[SymbolType]) -> bool {
  types.is_empty() || types.contains(&item.symbol_type)
}

fn role_matches(item: &OutlineItem, filters: &[RoleFilter]) -> bool {
  if filters.is_empty() {
    return item.roles.contains(&SymbolRole::Definition);
  }
  if filters.iter().any(|filter| filter.any) {
    return true;
  }
  filters
    .iter()
    .any(|filter| filter.roles.iter().all(|role| item.roles.contains(role)))
}

fn item_matches(item: &OutlineItem, arg: &OutlineArg, common: &OutlineCommonArg) -> bool {
  type_matches(item, &arg.symbol_type)
    && role_matches(item, &arg.role)
    && common_matches(item, common)
}

fn has_anchor_filters(arg: &OutlineArg, common: &OutlineCommonArg) -> bool {
  !arg.symbol_type.is_empty() || !common.matches.is_empty()
}

fn common_matches(item: &OutlineItem, common: &OutlineCommonArg) -> bool {
  common.matches.is_empty()
    || common.matches.iter().any(|regex| {
      item.name.as_ref().is_some_and(|name| regex.is_match(name))
        || item
          .signature
          .as_ref()
          .is_some_and(|signature| regex.is_match(signature))
        || regex.is_match(&item.source_line)
    })
}

fn print_outline(arg: &OutlineArg, files: Vec<OutlineFile>) -> Result<ExitCode> {
  let common = &arg.common;
  match common.json {
    None => print_text(&files),
    Some(JsonStyle::Pretty) => {
      println!("{}", serde_json::to_string_pretty(&files)?);
    }
    Some(JsonStyle::Compact) => {
      println!("{}", serde_json::to_string(&files)?);
    }
    Some(JsonStyle::Stream) => {
      for record in flatten_files(&files) {
        println!("{}", serde_json::to_string(&record)?);
      }
    }
  }
  Ok(ExitCode::SUCCESS)
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
  type_text_label(item.symbol_type)
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

fn type_text_label(symbol_type: SymbolType) -> &'static str {
  match symbol_type {
    SymbolType::EnumMember => "enum member",
    SymbolType::TypeParameter => "type parameter",
    _ => symbol_type.label(),
  }
}

fn flatten_files(files: &[OutlineFile]) -> Vec<OutlineRecord> {
  let mut records = vec![];
  for file in files {
    flatten_items_for_file(file, &file.items, None, &mut records);
  }
  records
}

fn flatten_items_for_file(
  file: &OutlineFile,
  items: &[OutlineItem],
  container: Option<OutlineContainer>,
  records: &mut Vec<OutlineRecord>,
) {
  for item in items {
    let current_container = Some(OutlineContainer {
      name: item.name.clone(),
      symbol_type: item.symbol_type,
      range: item.range.clone(),
    });
    records.push(OutlineRecord {
      path: file.path.clone(),
      language: file.language.clone(),
      symbol: flat_symbol(item, container.clone()),
    });
    flatten_items_for_file(file, &item.children, current_container, records);
  }
}

fn flat_symbol(item: &OutlineItem, container: Option<OutlineContainer>) -> OutlineFlatSymbol {
  OutlineFlatSymbol {
    name: item.name.clone(),
    symbol_type: item.symbol_type,
    roles: item.roles.clone(),
    range: item.range.clone(),
    signature: item.signature.clone(),
    child_digest: item.child_digest.clone(),
    ast_kind: item.ast_kind.clone(),
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(
      records
        .iter()
        .any(|r| r.symbol.name.as_deref() == Some("RunArg"))
    );
    assert!(
      records
        .iter()
        .all(|r| !r.symbol.roles.contains(&SymbolRole::Import))
    );
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(
      records
        .iter()
        .any(|r| r.symbol.roles.contains(&SymbolRole::Import))
    );
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
    let query = anchor_query("Parser", Some(SymbolType::Class));
    let mut files = vec![file];
    apply_view(&query, &mut files);
    assert_eq!(files[0].items.len(), 1);
    assert_eq!(files[0].items[0].name.as_deref(), Some("Parser"));
    assert!(
      files[0].items[0]
        .children
        .iter()
        .any(|child| child.name.as_deref() == Some("parse"))
    );
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
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
  fn ts_inner_functions_are_function_members() {
    let src = r#"
export function checkType() {
  function reportError() {}
  const compareTypes = () => true;
  const localValue = 1;
  return compareTypes();
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
    let query = anchor_query("checkType", Some(SymbolType::Function));
    let mut files = vec![file];
    apply_view(&query, &mut files);
    let function = files[0]
      .items
      .iter()
      .find(|item| item.name.as_deref() == Some("checkType"))
      .expect("checkType function");
    let names = function
      .children
      .iter()
      .filter_map(|item| item.name.as_deref())
      .collect::<Vec<_>>();
    assert_eq!(names, vec!["reportError", "compareTypes"]);
    assert!(!names.contains(&"localValue"));
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("java.util.List"))
    );
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Foo") && r.symbol.symbol_type == SymbolType::Class
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Rec") && r.symbol.symbol_type == SymbolType::Struct
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
    let query = anchor_query("Foo", Some(SymbolType::Class));
    let file = extract_outline(
      PathBuf::from("test.java"),
      SgLang::Builtin(SupportLang::Java),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("SIZE") && r.symbol.symbol_type == SymbolType::Constant
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("bar") && r.symbol.symbol_type == SymbolType::Method
    }));
    let query = local_exports_query();
    let file = extract_outline(
      PathBuf::from("test.java"),
      SgLang::Builtin(SupportLang::Java),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_view(&query, &mut files);
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
    let query = anchor_query("Foo", Some(SymbolType::Class));
    let mut files = vec![file];
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(
      records
        .iter()
        .any(|record| record.symbol.name.as_deref() == Some("name"))
    );
    assert!(
      records
        .iter()
        .any(|record| record.symbol.name.as_deref() == Some("bar"))
    );
    let query = local_exports_query();
    let file = extract_outline(
      PathBuf::from("test.kt"),
      SgLang::Builtin(SupportLang::Kotlin),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_view(&query, &mut files);
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(
      records
        .iter()
        .all(|r| r.symbol.name.as_deref() != Some("Foundation"))
    );
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Foo") && r.symbol.symbol_type == SymbolType::Class
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Box") && r.symbol.symbol_type == SymbolType::Struct
    }));
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Mode") && r.symbol.symbol_type == SymbolType::Enum
    }));
    let query = anchor_query("Foo", Some(SymbolType::Class));
    let file = extract_outline(
      PathBuf::from("test.swift"),
      SgLang::Builtin(SupportLang::Swift),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut files = vec![file];
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(
      records
        .iter()
        .any(|record| record.symbol.name.as_deref() == Some("name"))
    );
    assert!(
      records
        .iter()
        .any(|record| record.symbol.name.as_deref() == Some("bar"))
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    assert!(records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("Session") && r.symbol.symbol_type == SymbolType::Class
    }));
    assert!(!records.iter().any(|r| {
      r.symbol.name.as_deref() == Some("webSocketRequest")
        && r.symbol.symbol_type == SymbolType::Function
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
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
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
    let query = map_query_with_members(OutlineMembers::Lines);
    let mut files = vec![file];
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
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
    let query = anchor_query("RouterGroup", None);
    let mut files = vec![file];
    apply_view(&query, &mut files);
    let records = flatten_files(&files);
    let names = records
      .iter()
      .filter(|record| record.symbol.symbol_type == SymbolType::Method)
      .filter_map(|record| record.symbol.name.as_deref())
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
    apply_view(&query, &mut files);
    assert!(files[0].items.iter().all(|item| item.children.is_empty()));
  }

  #[test]
  fn members_lines_includes_members() {
    let src = "enum Commands { Run(RunArg) }\nstruct RunArg;\n";
    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let query = map_query_with_members(OutlineMembers::Lines);
    let mut files = vec![file];
    apply_view(&query, &mut files);
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
    roles: [definition]
    addRoles:
      export: textPrefix:pub
    name: field:name
    rule: { kind: function_item }
"#,
    )
    .expect("write outline rule");
    let mut common = test_common();
    common.no_default_outline_rules = true;
    common.outline_rules = vec![rule_path];
    let src = "pub struct RunArg {}\npub fn custom() {}\n";
    let file = extract_outline(
      PathBuf::from("test.rs"),
      SgLang::Builtin(SupportLang::Rust),
      src,
      &common,
      &load_outline_catalog(&common).expect("load outline catalog"),
    )
    .expect("extract outline");
    let records = flatten_files(&[file]);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].symbol.name.as_deref(), Some("custom"));
    assert_eq!(records[0].symbol.symbol_type, SymbolType::Function);
    assert_eq!(
      records[0].symbol.roles,
      vec![SymbolRole::Definition, SymbolRole::Export]
    );
  }

  #[test]
  fn direct_file_filter_keeps_empty_result() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file_path = dir.path().join("local.rs");
    std::fs::write(&file_path, "fn local() {}\n").expect("write source");
    let file = extract_outline(
      file_path.clone(),
      SgLang::Builtin(SupportLang::Rust),
      "fn local() {}\n",
      &test_common(),
      &test_catalog(),
    )
    .expect("extract outline");
    let mut query = local_exports_query();
    query.common.input.paths = vec![file_path];
    let mut files = vec![file];
    apply_view(&query, &mut files);
    assert_eq!(files.len(), 1);
    assert!(files[0].items.is_empty());
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
      json: None,
      matches: vec![],
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

  fn outline_arg_with_common(common: OutlineCommonArg) -> OutlineArg {
    OutlineArg {
      common,
      symbol_type: vec![],
      role: vec![],
      members: OutlineMembers::Names,
    }
  }

  fn map_query() -> OutlineArg {
    outline_arg_with_common(test_common())
  }

  fn map_query_with_members(members: OutlineMembers) -> OutlineArg {
    let mut arg = map_query();
    arg.members = members;
    arg
  }

  fn imports_query() -> OutlineArg {
    let mut arg = map_query();
    arg.role = vec![role_filter(&[SymbolRole::Import])];
    arg
  }

  fn local_exports_query() -> OutlineArg {
    let mut arg = map_query();
    arg.role = vec![role_filter(&[SymbolRole::Definition, SymbolRole::Export])];
    arg
  }

  fn anchor_query(name: &str, symbol_type: Option<SymbolType>) -> OutlineArg {
    let mut arg = map_query();
    arg.common.matches = vec![Regex::new(name).expect("test regex")];
    arg.symbol_type = symbol_type.into_iter().collect();
    arg.members = OutlineMembers::Lines;
    arg
  }

  fn role_filter(roles: &[SymbolRole]) -> RoleFilter {
    RoleFilter {
      any: false,
      roles: roles.to_vec(),
    }
  }
}
