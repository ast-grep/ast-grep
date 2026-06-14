use ast_grep_config::{SerializableRewriter, SerializableRule, SerializableRuleCore};
use serde::{Deserialize, Serialize};
use serde_yaml::{Deserializer, Error as YamlError, with::singleton_map_recursive::deserialize};

use super::model::SymbolType;

/// Serializable outline extractor definition loaded from an outline rule YAML document.
///
/// The `role` field selects the concrete rule shape. Item rules create top-level
/// entries. Member rules create direct child entries that can attach to eligible
/// item rules through `parentRuleIds`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "camelCase")]
enum SerializableOutlineRule {
  /// Top-level structure, like functions, classes, and imports.
  Item(SerializableItemRule),
  /// Direct child structure under an item, such as fields, methods, or variants.
  Member(SerializableMemberRule),
}

impl SerializableOutlineRule {
  fn common(&self) -> &SerializableOutlineCommon {
    match self {
      Self::Item(rule) => &rule.common,
      Self::Member(rule) => &rule.common,
    }
  }
}

/// Shared serializable fields for every outline extractor.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableOutlineCommon {
  /// Stable extractor id used in diagnostics and member parent references.
  id: String,
  /// Language name accepted by ast-grep, including built-in and registered custom languages.
  language: String,
  /// LSP-compatible outline category produced by this extractor.
  symbol_type: SymbolType,
  /// ast-grep rule-core fields used to select candidate syntax.
  #[serde(flatten)]
  matcher: SerializableRuleCore,
  /// Rewrite rules for `rewrite` transformation
  rewriters: Option<Vec<SerializableRewriter>>,
  /// Name template evaluated from metavariables or transformed metavariables.
  name: String,
  /// Optional source-like signature template. The extractor falls back to the
  /// first non-empty matched source line when omitted.
  signature: Option<String>,
}

/// Item extractor for top-level file/module structure.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableItemRule {
  /// Common outline extractor fields.
  #[serde(flatten)]
  common: SerializableOutlineCommon,
  /// Whether this item is an import/dependency edge.
  is_import: Option<SerializablePredicate>,
  /// Whether this item belongs to the file/module public surface.
  is_exported: Option<SerializablePredicate>,
}

/// Member extractor for direct child structure under an item.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableMemberRule {
  /// Common outline extractor fields.
  #[serde(flatten)]
  common: SerializableOutlineCommon,
  /// Eligible parent item extractor ids.
  parent_rule_ids: Vec<String>,
  /// Whether this member is syntactically public.
  is_public: Option<SerializablePredicate>,
}

/// Boolean derivation for outline flags.
///
/// A literal boolean sets the output flag directly. A rule object is evaluated
/// against the matched candidate node and sets the output flag from the match result.
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum SerializablePredicate {
  /// Literal boolean value.
  Literal(bool),
  /// ast-grep predicate evaluated against the extracted candidate node.
  Rule(Box<SerializableRule>),
}

/// Parse a stream of YAML outline extractor documents.
fn parse_outline_rules(src: &str) -> Result<Vec<SerializableOutlineRule>, YamlError> {
  Deserializer::from_str(src).map(deserialize).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_rule(src: &str) -> SerializableOutlineRule {
    ast_grep_config::from_str(src).expect("outline rule should deserialize")
  }

  #[test]
  fn deserializes_item_rule() {
    let rule = parse_rule(
      r#"
id: rust-struct
language: Rust
role: item
symbolType: struct
rule:
  pattern: $VIS struct $NAME { $$$BODY }
name: $NAME
isExported:
  has:
    regex: '^pub\b'
"#,
    );

    let SerializableOutlineRule::Item(item) = rule else {
      panic!("expected item rule");
    };
    assert_eq!(item.common.id, "rust-struct");
    assert_eq!(item.common.language, "Rust");
    assert_eq!(item.common.symbol_type, SymbolType::Struct);
    assert_eq!(item.common.name, "$NAME");
    assert!(matches!(
      item.is_exported,
      Some(SerializablePredicate::Rule(_))
    ));
    assert!(item.is_import.is_none());
  }

  #[test]
  fn deserializes_member_rule() {
    let rule = parse_rule(
      r#"
id: rust-field
language: Rust
role: member
parentRuleIds: [rust-struct]
symbolType: field
rule:
  pattern: '$VIS $NAME: $TYPE'
name: $NAME
signature: '$VIS $NAME: $TYPE'
isPublic:
  has:
    regex: '^pub\b'
"#,
    );

    let SerializableOutlineRule::Member(member) = rule else {
      panic!("expected member rule");
    };
    assert_eq!(member.common.id, "rust-field");
    assert_eq!(member.parent_rule_ids, vec!["rust-struct"]);
    assert_eq!(member.common.symbol_type, SymbolType::Field);
    assert_eq!(
      member.common.signature.as_deref(),
      Some("$VIS $NAME: $TYPE")
    );
    assert!(matches!(
      member.is_public,
      Some(SerializablePredicate::Rule(_))
    ));
  }

  #[test]
  fn deserializes_literal_booleans() {
    let rule = parse_rule(
      r#"
id: rust-use
language: Rust
role: item
symbolType: module
rule:
  pattern: use $TARGET;
name: $TARGET
isImport: true
isExported: false
"#,
    );

    let SerializableOutlineRule::Item(item) = rule else {
      panic!("expected item rule");
    };
    assert!(matches!(
      item.is_import,
      Some(SerializablePredicate::Literal(true))
    ));
    assert!(matches!(
      item.is_exported,
      Some(SerializablePredicate::Literal(false))
    ));
  }

  #[test]
  fn deserializes_transform_and_rewriters() {
    let rule = parse_rule(
      r#"
id: rust-use
language: Rust
role: item
symbolType: module
rule:
  pattern: use $TARGET;
transform:
  NAME:
    replace:
      source: $TARGET
      replace: '^.*::'
      by: ''
rewriters:
  - id: trim
    rule:
      pattern: $A
    fix: $A
name: $NAME
isImport: true
"#,
    );

    let SerializableOutlineRule::Item(item) = rule else {
      panic!("expected item rule");
    };
    assert_eq!(item.common.name, "$NAME");
    assert!(item.common.matcher.transform.is_some());
    assert_eq!(item.common.rewriters.as_ref().unwrap()[0].id, "trim");
  }

  #[test]
  fn parses_yaml_document_stream() {
    let rules = parse_outline_rules(
      r#"
id: rust-struct
language: Rust
role: item
symbolType: struct
rule:
  pattern: struct $NAME { $$$BODY }
name: $NAME
---
id: rust-field
language: Rust
role: member
parentRuleIds: [rust-struct]
symbolType: field
rule:
  pattern: '$NAME: $TYPE'
name: $NAME
"#,
    )
    .expect("document stream should deserialize");

    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].common().id, "rust-struct");
    assert_eq!(rules[1].common().id, "rust-field");
  }

  #[test]
  fn serializes_with_internal_role_tag() {
    let rule = SerializableOutlineRule::Item(SerializableItemRule {
      common: SerializableOutlineCommon {
        id: "ts-function".into(),
        language: "TypeScript".into(),
        symbol_type: SymbolType::Function,
        matcher: SerializableRuleCore {
          rule: ast_grep_config::from_str(
            r#"
pattern: function $NAME() { $$$BODY }
"#,
          )
          .expect("rule should deserialize"),
          constraints: None,
          utils: None,
          transform: None,
        },
        rewriters: None,
        name: "$NAME".into(),
        signature: Some("function $NAME()".into()),
      },
      is_import: None,
      is_exported: Some(SerializablePredicate::Literal(true)),
    });

    let value = serde_json::to_value(rule).expect("outline rule should serialize");

    assert_eq!(value["role"], "item");
    assert_eq!(value["id"], "ts-function");
    assert_eq!(value["symbolType"], "function");
    assert_eq!(value["isExported"], true);
  }
}
