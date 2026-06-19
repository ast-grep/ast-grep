//! Bundled outline extractor rules.
//!
//! Built-ins use the same YAML schema as user-provided `--outline-rules`
//! files. Keeping them data-driven preserves the same loading and execution path
//! for built-in and custom languages.

pub const DEFAULT_OUTLINE_RULES: &str = r#"
id: rust-use-public
language: Rust
role: item
symbolType: module
rule:
  pattern: 'pub use $PATH;'
name: '$PATH'
isImport: true
isExported: true
---
id: rust-use
language: Rust
role: item
symbolType: module
rule:
  pattern: 'use $PATH;'
name: '$PATH'
isImport: true
isExported: false
---
id: rust-mod-public
language: Rust
role: item
symbolType: module
rule:
  pattern: 'pub mod $NAME;'
name: '$NAME'
signature: 'pub mod $NAME'
isExported: true
---
id: rust-mod
language: Rust
role: item
symbolType: module
rule:
  pattern: 'mod $NAME;'
name: '$NAME'
signature: 'mod $NAME'
isExported: false
---
id: rust-mod-inline-public
language: Rust
role: item
symbolType: module
rule:
  pattern: 'pub mod $NAME { $$$BODY }'
name: '$NAME'
signature: 'pub mod $NAME'
isExported: true
---
id: rust-mod-inline
language: Rust
role: item
symbolType: module
rule:
  pattern: 'mod $NAME { $$$BODY }'
name: '$NAME'
signature: 'mod $NAME'
isExported: false
---
id: rust-function-visible
language: Rust
role: item
symbolType: function
rule:
  all:
    - kind: function_item
    - regex: '^pub\('
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: true
---
id: rust-function-public-generic-return-where
language: Rust
role: item
symbolType: function
rule:
  pattern: 'pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET where $$$WHERE { $$$BODY }'
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isExported: true
---
id: rust-function-generic-return-where
language: Rust
role: item
symbolType: function
rule:
  pattern: 'fn $NAME<$$$GENERICS>($$$ARGS) -> $RET where $$$WHERE { $$$BODY }'
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isExported: false
---
id: rust-function-public-generic-where
language: Rust
role: item
symbolType: function
rule:
  pattern: 'pub fn $NAME<$$$GENERICS>($$$ARGS) where $$$WHERE { $$$BODY }'
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS)'
isExported: true
---
id: rust-function-generic-where
language: Rust
role: item
symbolType: function
rule:
  pattern: 'fn $NAME<$$$GENERICS>($$$ARGS) where $$$WHERE { $$$BODY }'
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS)'
isExported: false
---
id: rust-function-public-generic-return
language: Rust
role: item
symbolType: function
rule:
  pattern: 'pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET { $$$BODY }'
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isExported: true
---
id: rust-function-generic-return
language: Rust
role: item
symbolType: function
rule:
  pattern: 'fn $NAME<$$$GENERICS>($$$ARGS) -> $RET { $$$BODY }'
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isExported: false
---
id: rust-function-public-generic
language: Rust
role: item
symbolType: function
rule:
  pattern: 'pub fn $NAME<$$$GENERICS>($$$ARGS) { $$$BODY }'
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS)'
isExported: true
---
id: rust-function-generic
language: Rust
role: item
symbolType: function
rule:
  pattern: 'fn $NAME<$$$GENERICS>($$$ARGS) { $$$BODY }'
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS)'
isExported: false
---
id: rust-function-public
language: Rust
role: item
symbolType: function
rule:
  pattern: 'pub fn $NAME($$$ARGS) { $$$BODY }'
name: '$NAME'
signature: 'pub fn $NAME($$$ARGS)'
isExported: true
---
id: rust-function
language: Rust
role: item
symbolType: function
rule:
  pattern: 'fn $NAME($$$ARGS) { $$$BODY }'
name: '$NAME'
signature: 'fn $NAME($$$ARGS)'
isExported: false
---
id: rust-function-public-return
language: Rust
role: item
symbolType: function
rule:
  pattern: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'
name: '$NAME'
signature: 'pub fn $NAME($$$ARGS) -> $RET'
isExported: true
---
id: rust-function-return
language: Rust
role: item
symbolType: function
rule:
  pattern: 'fn $NAME($$$ARGS) -> $RET { $$$BODY }'
name: '$NAME'
signature: 'fn $NAME($$$ARGS) -> $RET'
isExported: false
---
id: rust-struct-visible
language: Rust
role: item
symbolType: struct
rule:
  all:
    - kind: struct_item
    - regex: '^pub\('
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: true
---
id: rust-struct-public-any
language: Rust
role: item
symbolType: struct
rule:
  all:
    - kind: struct_item
    - regex: '^pub\s+struct'
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: true
---
id: rust-struct-any
language: Rust
role: item
symbolType: struct
rule:
  all:
    - kind: struct_item
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: false
---
id: rust-struct-public
language: Rust
role: item
symbolType: struct
rule:
  pattern: 'pub struct $NAME { $$$BODY }'
name: '$NAME'
signature: 'pub struct $NAME'
isExported: true
---
id: rust-struct
language: Rust
role: item
symbolType: struct
rule:
  pattern: 'struct $NAME { $$$BODY }'
name: '$NAME'
signature: 'struct $NAME'
isExported: false
---
id: rust-enum-visible
language: Rust
role: item
symbolType: enum
rule:
  all:
    - kind: enum_item
    - regex: '^pub\('
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: true
---
id: rust-enum-public-any
language: Rust
role: item
symbolType: enum
rule:
  all:
    - kind: enum_item
    - regex: '^pub\s+enum'
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: true
---
id: rust-enum-any
language: Rust
role: item
symbolType: enum
rule:
  all:
    - kind: enum_item
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isExported: false
---
id: rust-enum-public
language: Rust
role: item
symbolType: enum
rule:
  pattern: 'pub enum $NAME { $$$BODY }'
name: '$NAME'
signature: 'pub enum $NAME'
isExported: true
---
id: rust-enum
language: Rust
role: item
symbolType: enum
rule:
  pattern: 'enum $NAME { $$$BODY }'
name: '$NAME'
signature: 'enum $NAME'
isExported: false
---
id: rust-trait-public
language: Rust
role: item
symbolType: interface
rule:
  pattern: 'pub trait $NAME { $$$BODY }'
name: '$NAME'
signature: 'pub trait $NAME'
isExported: true
---
id: rust-trait
language: Rust
role: item
symbolType: interface
rule:
  pattern: 'trait $NAME { $$$BODY }'
name: '$NAME'
signature: 'trait $NAME'
isExported: false
---
id: rust-impl-trait-generic
language: Rust
role: item
symbolType: object
rule:
  pattern: 'impl<$$$GENERICS> $TRAIT for $NAME { $$$BODY }'
name: '$NAME'
signature: 'impl<$$$GENERICS> $TRAIT for $NAME'
isExported: false
---
id: rust-impl-trait
language: Rust
role: item
symbolType: object
rule:
  pattern: 'impl $TRAIT for $NAME { $$$BODY }'
name: '$NAME'
signature: 'impl $TRAIT for $NAME'
isExported: false
---
id: rust-impl-generic
language: Rust
role: item
symbolType: object
rule:
  pattern: 'impl<$$$GENERICS> $NAME { $$$BODY }'
name: '$NAME'
signature: 'impl<$$$GENERICS> $NAME'
isExported: false
---
id: rust-impl
language: Rust
role: item
symbolType: object
rule:
  pattern: 'impl $NAME { $$$BODY }'
name: '$NAME'
signature: 'impl $NAME'
isExported: false
---
id: rust-const-public
language: Rust
role: item
symbolType: constant
rule:
  pattern: 'pub const $NAME: $TYPE = $VALUE;'
name: '$NAME'
signature: 'pub const $NAME: $TYPE'
isExported: true
---
id: rust-const
language: Rust
role: item
symbolType: constant
rule:
  pattern: 'const $NAME: $TYPE = $VALUE;'
name: '$NAME'
signature: 'const $NAME: $TYPE'
isExported: false
---
id: rust-static-public
language: Rust
role: item
symbolType: variable
rule:
  pattern: 'pub static $NAME: $TYPE = $VALUE;'
name: '$NAME'
signature: 'pub static $NAME: $TYPE'
isExported: true
---
id: rust-static
language: Rust
role: item
symbolType: variable
rule:
  pattern: 'static $NAME: $TYPE = $VALUE;'
name: '$NAME'
signature: 'static $NAME: $TYPE'
isExported: false
---
id: rust-field
language: Rust
role: member
parentRuleIds: [rust-struct-visible, rust-struct-public-any, rust-struct-any, rust-struct-public, rust-struct]
symbolType: field
rule:
  kind: field_declaration
  has:
    field: name
    pattern: $NAME
name: '$NAME'
isPublic:
  regex: '^pub\b'
---
id: rust-enum-variant
language: Rust
role: member
parentRuleIds: [rust-enum-visible, rust-enum-public-any, rust-enum-any, rust-enum-public, rust-enum]
symbolType: enumMember
rule:
  kind: enum_variant
  has:
    field: name
    pattern: $NAME
name: '$NAME'
signature: '$NAME'
isPublic: true
---
id: rust-mod-member-function-public
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: function
rule:
  pattern:
    context: 'mod A { pub fn $NAME($$$ARGS) { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME($$$ARGS)'
isPublic: true
---
id: rust-mod-member-function
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: function
rule:
  pattern:
    context: 'mod A { fn $NAME($$$ARGS) { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME($$$ARGS)'
isPublic: false
---
id: rust-mod-member-function-public-return
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: function
rule:
  pattern:
    context: 'mod A { pub fn $NAME($$$ARGS) -> $RET { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME($$$ARGS) -> $RET'
isPublic: true
---
id: rust-mod-member-function-return
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: function
rule:
  pattern:
    context: 'mod A { fn $NAME($$$ARGS) -> $RET { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME($$$ARGS) -> $RET'
isPublic: false
---
id: rust-mod-member-struct-public
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: struct
rule:
  pattern:
    context: 'mod A { pub struct $NAME { $$$BODY } }'
    selector: struct_item
name: '$NAME'
signature: 'pub struct $NAME'
isPublic: true
---
id: rust-mod-member-struct
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: struct
rule:
  pattern:
    context: 'mod A { struct $NAME { $$$BODY } }'
    selector: struct_item
name: '$NAME'
signature: 'struct $NAME'
isPublic: false
---
id: rust-mod-member-enum-public
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: enum
rule:
  pattern:
    context: 'mod A { pub enum $NAME { $$$BODY } }'
    selector: enum_item
name: '$NAME'
signature: 'pub enum $NAME'
isPublic: true
---
id: rust-mod-member-enum
language: Rust
role: member
parentRuleIds: [rust-mod-inline-public, rust-mod-inline]
symbolType: enum
rule:
  pattern:
    context: 'mod A { enum $NAME { $$$BODY } }'
    selector: enum_item
name: '$NAME'
signature: 'enum $NAME'
isPublic: false
---
id: rust-method-public
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { pub fn $NAME($$$ARGS) { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME($$$ARGS)'
isPublic: true
---
id: rust-method
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { fn $NAME($$$ARGS) { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME($$$ARGS)'
isPublic: false
---
id: rust-method-public-return
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { pub fn $NAME($$$ARGS) -> $RET { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME($$$ARGS) -> $RET'
isPublic: true
---
id: rust-method-return
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { fn $NAME($$$ARGS) -> $RET { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME($$$ARGS) -> $RET'
isPublic: false
---
id: rust-method-visible
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  all:
    - kind: function_item
    - regex: '^pub\('
    - has:
        field: name
        pattern: $NAME
name: '$NAME'
isPublic: true
---
id: rust-method-public-generic-return-where
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET where $$$WHERE { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isPublic: true
---
id: rust-method-generic-return-where
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { fn $NAME<$$$GENERICS>($$$ARGS) -> $RET where $$$WHERE { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isPublic: false
---
id: rust-method-public-generic-where
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { pub fn $NAME<$$$GENERICS>($$$ARGS) where $$$WHERE { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS)'
isPublic: true
---
id: rust-method-generic-where
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { fn $NAME<$$$GENERICS>($$$ARGS) where $$$WHERE { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS)'
isPublic: false
---
id: rust-method-public-generic
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { pub fn $NAME<$$$GENERICS>($$$ARGS) { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS)'
isPublic: true
---
id: rust-method-generic
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { fn $NAME<$$$GENERICS>($$$ARGS) { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS)'
isPublic: false
---
id: rust-method-public-generic-return
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'pub fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isPublic: true
---
id: rust-method-generic-return
language: Rust
role: member
parentRuleIds: [rust-impl-trait-generic, rust-impl-trait, rust-impl-generic, rust-impl, rust-trait-public, rust-trait]
symbolType: method
rule:
  pattern:
    context: 'impl A { fn $NAME<$$$GENERICS>($$$ARGS) -> $RET { $$$BODY } }'
    selector: function_item
name: '$NAME'
signature: 'fn $NAME<$$$GENERICS>($$$ARGS) -> $RET'
isPublic: false
"#;

#[cfg(test)]
mod tests {
  use super::DEFAULT_OUTLINE_RULES;
  use crate::{
    combined_extractor::CombinedExtractors, extractor::parse_outline_rules, model::SymbolType,
  };
  use ast_grep_core::tree_sitter::LanguageExt;
  use ast_grep_language::SupportLang;

  fn rust_combined() -> CombinedExtractors<SupportLang> {
    let rules = parse_outline_rules::<SupportLang>(DEFAULT_OUTLINE_RULES)
      .expect("builtin outline rules should deserialize")
      .into_iter()
      .filter(|rule| rule.common().language == SupportLang::Rust)
      .collect::<Vec<_>>();
    CombinedExtractors::try_from(rules, &Default::default()).expect("rules should compile")
  }

  #[test]
  fn rust_builtin_rules_extract_file_outline() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
pub use crate::api::Parser;
use std::fmt;

pub struct Config {
  pub name: String,
  enabled: bool,
}

enum Mode {
  Fast,
  Slow,
  RuleConfig(#[from] RuleConfigError),
  Predicate(#[from] RuleSerializeError),
  Template(#[from] TemplateFixError),
  Complex {
    /// nth-child syntax
    position: NthChildSimple,
    /// select the nth node that matches the rule, like CSS's of syntax
    of_rule: Option<Box<SerializableRule>>,
    /// matches from the end instead like CSS's nth-last-child
    #[serde(default)]
    reverse: bool,
  },
}

impl Config {
  pub fn new(name: String) -> Self {
    Self { name, enabled: true }
  }

  fn enabled(&self) -> bool {
    self.enabled
  }
}

fn helper() {}

mod tests {
  fn nested_helper() {}

  #[test]
  fn parses_config() {}
}
"#,
    );

    let items = combined.extract(grep.root());
    let names = items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(
      names,
      vec![
        "crate::api::Parser",
        "std::fmt",
        "Config",
        "Mode",
        "Config",
        "helper",
        "tests"
      ]
    );

    let config = items
      .iter()
      .find(|item| item.entry.name == "Config" && item.entry.symbol_type == SymbolType::Struct)
      .expect("Config struct should be extracted");
    let fields = config
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.is_public))
      .collect::<Vec<_>>();
    assert_eq!(fields, vec![("name", true), ("enabled", false)]);

    let mode = items
      .iter()
      .find(|item| item.entry.name == "Mode")
      .expect("Mode enum should be extracted");
    let variants = mode
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.entry.signature.as_ref()))
      .collect::<Vec<_>>();
    assert_eq!(
      variants,
      vec![
        ("Fast", "Fast"),
        ("Slow", "Slow"),
        ("RuleConfig", "RuleConfig"),
        ("Predicate", "Predicate"),
        ("Template", "Template"),
        ("Complex", "Complex")
      ]
    );

    let implementation = items
      .iter()
      .find(|item| item.entry.name == "Config" && item.entry.symbol_type == SymbolType::Object)
      .expect("Config impl should be extracted");
    let methods = implementation
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.is_public))
      .collect::<Vec<_>>();
    assert_eq!(methods, vec![("new", true), ("enabled", false)]);

    let tests = items
      .iter()
      .find(|item| item.entry.name == "tests")
      .expect("inline test module should be extracted");
    let members = tests
      .members
      .iter()
      .map(|member| (member.entry.symbol_type, member.entry.name.as_ref()))
      .collect::<Vec<_>>();
    assert_eq!(
      members,
      vec![
        (SymbolType::Function, "nested_helper"),
        (SymbolType::Function, "parses_config")
      ]
    );
  }

  #[test]
  fn rust_builtin_rules_scope_inline_modules_and_impls() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
mod tests {
  pub fn public_case() {}
  fn helper() -> bool { false }
  struct Fixture {}
  enum Mode { A }
}

trait Service {}

impl Service for Config {
  fn run(&self) {}
}

impl<T> Box<T> {
  pub fn value(&self) -> &T { todo!() }
}

impl Rewrite<String> {
  pub fn parse<L: Language>(&self, lang: &L) -> Result<Rewrite<MetaVariable>, TransformError> {
    todo!()
  }
}
"#,
    );

    let items = combined.extract(grep.root());
    let names = items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(
      names,
      vec!["tests", "Service", "Config", "Box<T>", "Rewrite<String>"]
    );

    let tests = items
      .iter()
      .find(|item| item.entry.name == "tests")
      .expect("inline test module should be extracted");
    let module_members = tests
      .members
      .iter()
      .map(|member| (member.entry.symbol_type, member.entry.name.as_ref()))
      .collect::<Vec<_>>();
    assert_eq!(
      module_members,
      vec![
        (SymbolType::Function, "public_case"),
        (SymbolType::Function, "helper"),
        (SymbolType::Struct, "Fixture"),
        (SymbolType::Enum, "Mode")
      ]
    );

    let trait_impl = items
      .iter()
      .find(|item| item.entry.signature == "impl Service for Config")
      .expect("trait impl should be extracted");
    let trait_impl_methods = trait_impl
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(trait_impl_methods, vec!["run"]);

    let generic_impl = items
      .iter()
      .find(|item| item.entry.signature == "impl<T> Box<T>")
      .expect("generic impl should be extracted");
    let generic_impl_methods = generic_impl
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(generic_impl_methods, vec!["value"]);

    let rewrite_impl = items
      .iter()
      .find(|item| item.entry.signature == "impl Rewrite<String>")
      .expect("impl with type arguments should be extracted");
    let rewrite_methods = rewrite_impl
      .members
      .iter()
      .map(|member| {
        (
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      rewrite_methods,
      vec![(
        "parse",
        "pub fn parse<L: Language>(&self, lang: &L) -> Result<Rewrite<MetaVariable>, TransformError>",
        true
      )]
    );
  }

  #[test]
  fn rust_builtin_rules_extract_tokio_declaration_shapes() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
pub(super) struct Cell<T: Future, S> {
  pub(super) header: Header,
  core: Core<T, S>,
}

pub(crate) struct Launch(Vec<Arc<Worker>>);

struct Local<T>(T);

pub(super) enum Scheduler<T> {
  CurrentThread(T),
  MultiThread,
}

enum Stage<T: Future> {
  Running(T),
  Finished,
}

pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  todo!()
}

pub(crate) fn block_in_place<F, R>(f: F) -> R
where
  F: FnOnce() -> R,
{
  f()
}

fn with_current<R>(f: impl FnOnce(Option<&Context>) -> R) -> R {
  f(None)
}

impl<T: Future> CoreStage<T> {
  pub(super) fn with_mut<R>(&self, f: impl FnOnce(*mut Stage<T>) -> R) -> R {
    todo!()
  }

  fn with_core<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut Core) -> R,
  {
    todo!()
  }
}
"#,
    );

    let items = combined.extract(grep.root());
    let item_shapes = items
      .iter()
      .map(|item| {
        (
          item.entry.symbol_type,
          item.entry.name.as_ref(),
          item.is_exported,
        )
      })
      .collect::<Vec<_>>();

    assert_eq!(
      item_shapes,
      vec![
        (SymbolType::Struct, "Cell", true),
        (SymbolType::Struct, "Launch", true),
        (SymbolType::Struct, "Local", false),
        (SymbolType::Enum, "Scheduler", true),
        (SymbolType::Enum, "Stage", false),
        (SymbolType::Function, "spawn", true),
        (SymbolType::Function, "block_in_place", true),
        (SymbolType::Function, "with_current", false),
        (SymbolType::Object, "CoreStage<T>", false),
      ]
    );

    let scheduler = items
      .iter()
      .find(|item| item.entry.name == "Scheduler")
      .expect("restricted visibility generic enum should be extracted");
    let variants = scheduler
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(variants, vec!["CurrentThread", "MultiThread"]);

    let implementation = items
      .iter()
      .find(|item| item.entry.name == "CoreStage<T>")
      .expect("generic impl should be extracted");
    let methods = implementation
      .members
      .iter()
      .map(|member| {
        (
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      methods,
      vec![
        (
          "with_mut",
          "pub(super) fn with_mut<R>(&self, f: impl FnOnce(*mut Stage<T>) -> R) -> R {",
          true
        ),
        ("with_core", "fn with_core<F, R>(&self, f: F) -> R", false),
      ]
    );
  }
}
