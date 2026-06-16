//! Serializable outline result model.
//!
//! Outline has two placement roles. `Item` represents file/module-level
//! structure. `Member` represents direct structure inside an item.
//! The model intentionally stops at this item/member boundary. It preserves the
//! source shape needed for navigation and filtering, but does not try to build a
//! semantic graph of references, inheritance, or implemented protocols.

use std::borrow::Cow;
use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Outline symbol category.
///
/// The names follow LSP `DocumentSymbol.kind`, but ast-grep stores the symbolic
/// category directly instead of exposing LSP numeric values.
/// See https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/#textDocument_documentSymbol
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolType {
  File,
  Module,
  Namespace,
  Package,
  Class,
  Method,
  Property,
  Field,
  Constructor,
  Enum,
  Interface,
  Function,
  Variable,
  Constant,
  String,
  Number,
  Boolean,
  Array,
  Object,
  Key,
  Null,
  EnumMember,
  Struct,
  Event,
  Operator,
  TypeParameter,
}

/// Entry placement in the outline tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryRole {
  /// Top-level structure, such as `struct Foo`, `class Parser`, or `import ...`.
  Item,
  /// Direct child structure under an item, such as a field, method, or variant.
  Member,
}

/// Zero-based character position in a file.
///
/// This mirrors scan JSON's private `Position` shape. Core `Position` is not
/// serializable, and config's serializable range type is an internal matcher
/// input shape with optional columns, so outline keeps its output contract local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePosition {
  /// Zero-based line number.
  pub line: usize,
  /// Zero-based character column in the line.
  pub column: usize,
}

/// Source range for an outline entry.
///
/// This mirrors scan JSON's private `Range` shape. Outline owns the type here
/// so the model does not depend on scan rendering internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
  /// Inclusive start and exclusive end byte offsets.
  pub byte_offset: Range<usize>,
  pub start: SourcePosition,
  pub end: SourcePosition,
}

/// Shared structural data for either a top-level item or a direct member.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineEntry<'a> {
  pub role: EntryRole,
  pub symbol_type: SymbolType,
  pub name: Cow<'a, str>,
  pub range: SourceRange,
  pub signature: Cow<'a, str>,
  pub ast_kind: Cow<'a, str>,
}

/// One top-level outline item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItem<'a> {
  #[serde(flatten)]
  pub entry: OutlineEntry<'a>,
  pub is_import: bool,
  pub is_exported: bool,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub members: Vec<OutlineMember<'a>>,
}

/// One direct member under an outline item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineMember<'a> {
  #[serde(flatten)]
  pub entry: OutlineEntry<'a>,
  pub is_public: bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  fn text(value: &'static str) -> Cow<'static, str> {
    Cow::Borrowed(value)
  }

  fn test_range() -> SourceRange {
    SourceRange {
      byte_offset: 0..12,
      start: SourcePosition { line: 0, column: 0 },
      end: SourcePosition {
        line: 0,
        column: 12,
      },
    }
  }

  fn entry(
    role: EntryRole,
    symbol_type: SymbolType,
    name: &'static str,
    signature: &'static str,
    ast_kind: &'static str,
  ) -> OutlineEntry<'static> {
    OutlineEntry {
      role,
      symbol_type,
      name: text(name),
      range: test_range(),
      signature: text(signature),
      ast_kind: text(ast_kind),
    }
  }

  fn item(
    symbol_type: SymbolType,
    name: &'static str,
    signature: &'static str,
    ast_kind: &'static str,
  ) -> OutlineItem<'static> {
    OutlineItem {
      entry: entry(EntryRole::Item, symbol_type, name, signature, ast_kind),
      is_import: false,
      is_exported: false,
      members: vec![],
    }
  }

  fn member(
    symbol_type: SymbolType,
    name: &'static str,
    signature: &'static str,
    ast_kind: &'static str,
  ) -> OutlineMember<'static> {
    OutlineMember {
      entry: entry(EntryRole::Member, symbol_type, name, signature, ast_kind),
      is_public: false,
    }
  }

  #[test]
  fn serializes_and_deserializes_outline_contract() {
    let mut item = item(
      SymbolType::Class,
      "Parser",
      "export class Parser {",
      "class_declaration",
    );
    item.is_exported = true;
    item.members = vec![member(
      SymbolType::Method,
      "parse",
      "parse(input: string) {",
      "method_definition",
    )];

    let json = serde_json::to_value(&item).expect("outline item should serialize");

    assert_eq!(json["role"], "item");
    assert_eq!(json["symbolType"], "class");
    assert_eq!(json["name"], "Parser");
    assert_eq!(json["isImport"], false);
    assert_eq!(json["isExported"], true);
    assert_eq!(json["members"][0]["role"], "member");
    assert_eq!(json["members"][0]["symbolType"], "method");
  }
}
