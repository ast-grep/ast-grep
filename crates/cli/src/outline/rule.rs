use std::fmt::{Display, Formatter};

use ast_grep_config::SerializableRuleCore;
use serde::Deserialize;

use crate::lang::SgLang;
use crate::outline::model::{OutlineRole, OutlineRoles, SymbolType};

/// Serialized outline extractor definition.
///
/// The `rule`, `constraints`, `utils`, and `transform` fields are flattened
/// from ast-grep's existing rule core. Outline extraction reuses ast-grep rules
/// instead of adding a tree-sitter query format.
///
/// The matched node's source range becomes the outline item range. The `name`
/// field is an ast-grep template string used to derive the display name from
/// the match. For example, `"$NAME"` can reference a meta-variable captured by
/// the rule.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableOutlineRule {
  /// Unique, descriptive identifier for this extractor.
  id: String,
  /// Language this extractor applies to.
  language: SgLang,
  /// Symbol category assigned to matches from this extractor.
  symbol_type: SymbolType,
  /// File-level roles assigned to matches from this extractor.
  roles: Vec<OutlineRole>,
  /// ast-grep template string used to compute the outline item name.
  name: String,
  /// ast-grep rule core used to find outline items.
  #[serde(flatten)]
  core: SerializableRuleCore,
}

/// Serialized outline extractor collection.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableOutlineRuleSet {
  /// Extractors to load.
  extractors: Vec<SerializableOutlineRule>,
}

/// Normalized outline extractor definition.
#[derive(Clone)]
struct OutlineRule {
  /// Unique, descriptive identifier for this extractor.
  id: String,
  /// Language this extractor applies to.
  language: SgLang,
  /// Symbol category assigned to matches from this extractor.
  symbol_type: SymbolType,
  /// File-level roles assigned to matches from this extractor.
  roles: OutlineRoles,
  /// ast-grep template string used to compute the outline item name.
  name: String,
  /// ast-grep rule core used to find outline items.
  ///
  /// TODO: Parse this into a runtime `RuleCore` when the extraction runtime is
  /// added.
  core: SerializableRuleCore,
}

/// Error produced while normalizing outline extractor definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OutlineRuleError {
  /// The extractor did not declare any role.
  EmptyRoles(String),
}

impl Display for OutlineRuleError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      OutlineRuleError::EmptyRoles(id) => {
        write!(f, "outline extractor `{id}` must specify at least one role")
      }
    }
  }
}

impl std::error::Error for OutlineRuleError {}

impl TryFrom<SerializableOutlineRule> for OutlineRule {
  type Error = OutlineRuleError;

  fn try_from(rule: SerializableOutlineRule) -> Result<Self, Self::Error> {
    let roles = OutlineRoles::new(rule.roles);
    if roles.is_empty() {
      return Err(OutlineRuleError::EmptyRoles(rule.id));
    }
    Ok(Self {
      id: rule.id,
      language: rule.language,
      symbol_type: rule.symbol_type,
      roles,
      name: rule.name,
      core: rule.core,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use ast_grep_config::from_str;

  fn parse_rule_set(source: &str) -> SerializableOutlineRuleSet {
    from_str(source).expect("outline rules should parse")
  }

  #[test]
  fn parses_outline_rule_contract() {
    let rules = parse_rule_set(
      r#"
extractors:
  - id: rust-function
    language: Rust
    symbolType: function
    roles: [definition, export]
    name: "$NAME"
    rule:
      pattern: fn $NAME() {}
"#,
    );
    let rule = rules.extractors.into_iter().next().expect("one rule");
    let rule = OutlineRule::try_from(rule).expect("roles should normalize");

    assert_eq!(rule.id, "rust-function");
    assert_eq!(rule.symbol_type, SymbolType::Function);
    assert!(rule.roles.contains(OutlineRole::Definition));
    assert!(rule.roles.contains(OutlineRole::Export));
    assert!(!rule.roles.contains(OutlineRole::Import));
    assert_eq!(rule.name, "$NAME");
  }

  #[test]
  fn parses_name_template() {
    let rules = parse_rule_set(
      r#"
extractors:
  - id: rust-use
    language: Rust
    symbolType: module
    roles: [import]
    name: "$MATCH"
    rule:
      kind: use_declaration
"#,
    );
    let rule = rules.extractors.into_iter().next().expect("one rule");

    assert_eq!(rule.name, "$MATCH");
  }

  #[test]
  fn rejects_empty_roles() {
    let rules = parse_rule_set(
      r"
extractors:
  - id: rust-function
    language: Rust
    symbolType: function
    roles: []
    name: text
    rule:
      kind: function_item
",
    );
    let rule = rules.extractors.into_iter().next().expect("one rule");
    let error = match OutlineRule::try_from(rule) {
      Err(error) => error,
      Ok(_) => panic!("empty roles should fail"),
    };

    assert!(matches!(error, OutlineRuleError::EmptyRoles(id) if id == "rust-function"));
  }
}
