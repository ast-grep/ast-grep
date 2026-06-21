//! Rule loading and compilation for outline extraction.
//!
//! Outline rule files describe extractors in a YAML-friendly shape. This module
//! converts that schema into runtime rules with compiled ast-grep matchers,
//! templates, and predicates. Traversal code should depend on the runtime
//! `Outline*Rule` types, not on serde defaults or config parsing details.

use ast_grep_config::{
  GlobalRules, Rule, RuleConfig, RuleConfigError, RuleSerializeError, SerializableRewriter,
  SerializableRule, SerializableRuleConfig, SerializableRuleCore, Severity,
};
use ast_grep_core::{
  Doc, Language, Node, NodeMatch,
  matcher::{Matcher, MatcherExt},
  replacer::{Replacer, TemplateFix, TemplateFixError},
  source::Content,
};
use serde::{Deserialize, Serialize};
use serde_yaml::{Deserializer, Error as YamlError, with::singleton_map_recursive::deserialize};
use std::borrow::Cow;
use thiserror::Error;

use crate::model::{
  EntryRole, OutlineEntry, OutlineItem, OutlineMember, SourcePosition, SourceRange, SymbolType,
};
use crate::options::OutlineEntryDetail;

/// Serializable outline extractor definition loaded from an outline rule YAML document.
///
/// The `role` field selects the concrete rule shape. Item rules create top-level
/// entries. Member rules create direct child entries that can attach to eligible
/// item rules through `parentRuleIds`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "camelCase")]
pub enum SerializableOutlineRule<L> {
  /// Top-level structure, like functions, classes, and imports.
  Item(SerializableItemRule<L>),
  /// Direct child structure under an item, such as fields, methods, or variants.
  Member(SerializableMemberRule<L>),
}

impl<L> SerializableOutlineRule<L> {
  pub fn common(&self) -> &SerializableOutlineCommon<L> {
    match self {
      Self::Item(rule) => &rule.common,
      Self::Member(rule) => &rule.common,
    }
  }
}

/// Shared serializable fields for every outline extractor.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableOutlineCommon<L> {
  /// Stable extractor id used in diagnostics and member parent references.
  pub id: String,
  /// Language accepted by ast-grep, including built-in and registered custom languages.
  pub language: L,
  /// LSP-compatible outline category produced by this extractor.
  pub symbol_type: SymbolType,
  /// ast-grep rule-core fields used to select candidate syntax.
  #[serde(flatten)]
  pub matcher: SerializableRuleCore,
  /// Rewrite rules for `rewrite` transformation.
  pub rewriters: Option<Vec<SerializableRewriter>>,
  /// Name template evaluated from metavariables or transformed metavariables.
  pub name: String,
  /// Optional source-like signature template. The extractor falls back to the
  /// first non-empty matched source line when omitted.
  pub signature: Option<String>,
}

impl<L: Language> SerializableOutlineCommon<L> {
  fn into_rule_config(self) -> SerializableRuleConfig<L> {
    SerializableRuleConfig {
      core: self.matcher,
      fix: None,
      rewriters: self.rewriters,
      id: self.id,
      language: self.language,
      message: String::new(),
      note: None,
      severity: Severity::default(),
      labels: None,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }
}

/// Item extractor for top-level file/module structure.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableItemRule<L> {
  /// Common outline extractor fields.
  #[serde(flatten)]
  pub common: SerializableOutlineCommon<L>,
  /// Whether this item is an import/dependency edge.
  pub is_import: Option<SerializablePredicate>,
  /// Whether this item belongs to the file/module public surface.
  pub is_exported: Option<SerializablePredicate>,
}

/// Member extractor for direct child structure under an item.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableMemberRule<L> {
  /// Common outline extractor fields.
  #[serde(flatten)]
  pub common: SerializableOutlineCommon<L>,
  /// Eligible parent item extractor ids.
  pub parent_rule_ids: Vec<String>,
  /// Whether this member is syntactically public.
  pub is_public: Option<SerializablePredicate>,
}

/// Boolean derivation for outline flags.
///
/// A literal boolean sets the output flag directly. A rule object is evaluated
/// against the matched candidate node and sets the output flag from the match result.
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SerializablePredicate {
  /// Literal boolean value.
  Literal(bool),
  /// ast-grep predicate evaluated against the extracted candidate node.
  Rule(Box<SerializableRule>),
}

/// Shared parsed fields for every runnable outline extractor.
pub struct ExtractorCommon<L: Language> {
  /// Parsed ast-grep rule config used to select candidate syntax.
  pub rule: RuleConfig<L>,
  /// LSP-compatible outline category produced by this extractor.
  pub symbol_type: SymbolType,
  /// Name template evaluated from metavariables or transformed metavariables.
  pub name: TemplateFix,
  /// Optional source-like signature template.
  pub signature: Option<TemplateFix>,
  /// Requested text detail for this entry.
  detail: OutlineEntryDetail,
}

#[derive(Debug, Error)]
pub enum OutlineRuleError {
  #[error(transparent)]
  RuleConfig(#[from] RuleConfigError),
  // TODO: this error message is vague
  #[error("Predicate rule is not correctly configured")]
  Predicate(#[from] RuleSerializeError),
  #[error(transparent)]
  Template(#[from] TemplateFixError),
  #[error("Member rule `{rule_id}` references unknown parent rule `{parent_id}`")]
  UnknownParentRuleId { rule_id: String, parent_id: String },
  #[error("Member rule `{rule_id}` cannot use member rule `{parent_id}` as a parent")]
  InvalidParentRuleRole { rule_id: String, parent_id: String },
}

impl<L: Language> ExtractorCommon<L> {
  pub fn try_from(
    common: SerializableOutlineCommon<L>,
    globals: &GlobalRules,
    detail: OutlineEntryDetail,
  ) -> Result<Self, OutlineRuleError> {
    let symbol_type = common.symbol_type;
    let transform_vars = transform_vars(&common.matcher);
    let compile = |tmpl| compile_template(tmpl, &common.language, &transform_vars);
    let name = compile(&common.name)?;
    let signature = match detail {
      OutlineEntryDetail::Name => None,
      OutlineEntryDetail::Signature => common.signature.as_deref().map(compile).transpose()?,
    };
    let rule = RuleConfig::try_from(common.into_rule_config(), globals)?;
    Ok(Self {
      rule,
      symbol_type,
      name,
      signature,
      detail,
    })
  }

  // this function is not inherently bound to ExtractorCommon
  // just for convenience to avoid env
  fn compile_predicate(
    &self,
    predicate: Option<SerializablePredicate>,
    default: bool,
  ) -> Result<OutlinePredicate, OutlineRuleError> {
    let Some(predicate) = predicate else {
      return Ok(OutlinePredicate::Literal(default));
    };
    let ret = match predicate {
      SerializablePredicate::Literal(value) => OutlinePredicate::Literal(value),
      SerializablePredicate::Rule(rule) => {
        let env = self.rule.matcher.get_env(self.rule.language.clone());
        OutlinePredicate::Rule(env.deserialize_rule(*rule)?)
      }
    };
    Ok(ret)
  }
}

fn transform_vars(matcher: &SerializableRuleCore) -> Option<Vec<String>> {
  matcher
    .transform
    .as_ref()
    .map(|transform| transform.keys().cloned().collect())
}

fn compile_template<L: Language>(
  template: &str,
  language: &L,
  transform_vars: &Option<Vec<String>>,
) -> Result<TemplateFix, TemplateFixError> {
  if let Some(vars) = transform_vars {
    Ok(TemplateFix::with_transform(template, language, vars))
  } else {
    TemplateFix::try_new(template, language)
  }
}

// imported/exported will be default accordingly to role
enum OutlinePredicate {
  Literal(bool),
  Rule(Rule),
}

impl OutlinePredicate {
  fn evaluate<D: Doc>(&self, node_match: &NodeMatch<D>) -> bool {
    match self {
      Self::Literal(value) => *value,
      Self::Rule(rule) => {
        let mut env = Cow::Borrowed(node_match.get_env());
        // must use env to match main rule's metavariables
        rule
          .match_node_with_env(node_match.get_node().clone(), &mut env)
          .is_some()
      }
    }
  }
}

/// Runnable item extractor for top-level file/module structure.
pub struct ItemExtractor<L: Language> {
  pub common: ExtractorCommon<L>,
  is_import: OutlinePredicate,
  is_exported: OutlinePredicate,
}

impl<L: Language> ItemExtractor<L> {
  pub fn try_from(
    item: SerializableItemRule<L>,
    globals: &GlobalRules,
    detail: OutlineEntryDetail,
  ) -> Result<Self, OutlineRuleError> {
    let SerializableItemRule {
      common,
      is_import,
      is_exported,
    } = item;
    let common = ExtractorCommon::try_from(common, globals, detail)?;
    let is_import = common.compile_predicate(is_import, false)?;
    let is_exported = common.compile_predicate(is_exported, true)?;
    Ok(Self {
      common,
      is_import,
      is_exported,
    })
  }

  pub fn match_node<'tree, D: Doc>(&self, node: &Node<'tree, D>) -> Option<NodeMatch<'tree, D>> {
    self.common.rule.matcher.match_node(node.clone())
  }

  pub fn extract<'tree, D: Doc>(
    &self,
    node_match: &NodeMatch<'tree, D>,
    members: Vec<OutlineMember<'tree>>,
  ) -> OutlineItem<'tree> {
    OutlineItem {
      entry: self.common.extract_entry(EntryRole::Item, node_match),
      is_import: self.is_import.evaluate(node_match),
      is_exported: self.is_exported.evaluate(node_match),
      members,
    }
  }
}

/// Runnable member extractor for direct child structure under an item.
pub struct MemberExtractor<L: Language> {
  pub common: ExtractorCommon<L>,
  pub parent_rule_ids: Vec<String>,
  is_public: OutlinePredicate,
}

impl<L: Language> MemberExtractor<L> {
  pub fn try_from(
    member: SerializableMemberRule<L>,
    globals: &GlobalRules,
    detail: OutlineEntryDetail,
  ) -> Result<Self, OutlineRuleError> {
    let SerializableMemberRule {
      common,
      parent_rule_ids,
      is_public,
    } = member;
    let common = ExtractorCommon::try_from(common, globals, detail)?;
    let is_public = common.compile_predicate(is_public, true)?;
    Ok(Self {
      common,
      parent_rule_ids,
      is_public,
    })
  }

  pub fn match_node<'tree, D: Doc>(&self, node: &Node<'tree, D>) -> Option<NodeMatch<'tree, D>> {
    self.common.rule.matcher.match_node(node.clone())
  }

  pub fn extract<'tree, D: Doc>(&self, node_match: &NodeMatch<'tree, D>) -> OutlineMember<'tree> {
    OutlineMember {
      entry: self.common.extract_entry(EntryRole::Member, node_match),
      is_public: self.is_public.evaluate(node_match),
    }
  }
}

impl<L: Language> ExtractorCommon<L> {
  fn extract_entry<'tree, D: Doc>(
    &self,
    role: EntryRole,
    node_match: &NodeMatch<'tree, D>,
  ) -> OutlineEntry<'tree> {
    let node = node_match.get_node();
    OutlineEntry {
      role,
      symbol_type: self.symbol_type,
      name: render_template(&self.name, node_match).into(),
      range: source_range(node),
      signature: self.render_signature(node_match).into(),
      ast_kind: node.kind().into_owned().into(),
    }
  }

  fn render_signature<'tree, D: Doc>(&self, node_match: &NodeMatch<'tree, D>) -> String {
    match self.detail {
      OutlineEntryDetail::Name => String::new(),
      OutlineEntryDetail::Signature => self
        .signature
        .as_ref()
        .map(|template| render_template(template, node_match))
        .unwrap_or_else(|| default_signature(node_match.get_node())),
    }
  }
}

fn render_template<D: Doc>(template: &TemplateFix, node_match: &NodeMatch<D>) -> String {
  let bytes = template.generate_replacement(node_match);
  <D::Source as Content>::encode_bytes(&bytes).to_string()
}

fn default_signature<D: Doc>(node: &Node<D>) -> String {
  node
    .text()
    .lines()
    .find_map(|line| {
      let trimmed = line.trim();
      (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
    .unwrap_or_default()
}

fn source_range<D: Doc>(node: &Node<D>) -> SourceRange {
  let start = node.start_pos();
  let end = node.end_pos();
  SourceRange {
    byte_offset: node.range(),
    start: SourcePosition {
      line: start.line(),
      column: start.column(node),
    },
    end: SourcePosition {
      line: end.line(),
      column: end.column(node),
    },
  }
}

/// Parse a stream of YAML outline extractor documents.
pub fn parse_outline_rules<'a, L>(
  src: &'a str,
) -> Result<Vec<SerializableOutlineRule<L>>, YamlError>
where
  L: Deserialize<'a>,
{
  Deserializer::from_str(src).map(deserialize).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use ast_grep_core::tree_sitter::LanguageExt;
  use ast_grep_language::SupportLang;

  fn parse_rule(src: &str) -> SerializableOutlineRule<SupportLang> {
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
    assert_eq!(item.common.language, SupportLang::Rust);
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
    let rules = parse_outline_rules::<SupportLang>(
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
  fn parses_outline_common_to_runtime_rule() {
    let rule = parse_rule(
      r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() { $$$BODY }
name: $NAME
signature: function $NAME()
"#,
    );

    let SerializableOutlineRule::Item(item) = rule else {
      panic!("expected item rule");
    };
    let common = ExtractorCommon::try_from(
      item.common,
      &Default::default(),
      OutlineEntryDetail::Signature,
    )
    .expect("common rule should parse");

    assert_eq!(common.rule.id, "ts-function");
    assert_eq!(common.symbol_type, SymbolType::Function);
    assert!(common.name.used_vars().contains("NAME"));
    assert!(
      common
        .signature
        .as_ref()
        .is_some_and(|signature| signature.used_vars().contains("NAME"))
    );
  }

  #[test]
  fn parses_outline_item_rule() {
    let rule = parse_rule(
      r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() { $$$BODY }
name: $NAME
isImport: true
"#,
    );

    let SerializableOutlineRule::Item(item) = rule else {
      panic!("expected item rule");
    };
    let item = ItemExtractor::try_from(item, &Default::default(), OutlineEntryDetail::Signature)
      .expect("item rule should parse");

    assert_eq!(item.common.rule.id, "ts-function");
    assert!(matches!(item.is_import, OutlinePredicate::Literal(true)));
    assert!(matches!(item.is_exported, OutlinePredicate::Literal(true)));
  }

  #[test]
  fn predicate_rule_reuses_match_metavariables() {
    let rule = parse_rule(
      r#"
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  pattern: class $NAME { $$$BODY }
name: $NAME
isExported:
  has:
    pattern:
      context: class A { $NAME() { $$$BODY } }
      selector: method_definition
"#,
    );

    let SerializableOutlineRule::Item(item) = rule else {
      panic!("expected item rule");
    };
    let item = ItemExtractor::try_from(item, &Default::default(), OutlineEntryDetail::Signature)
      .expect("item rule should parse");
    let root = SupportLang::TypeScript.ast_grep("class Foo { bar() {} }");
    let class_node = root
      .root()
      .children()
      .find(|node| node.kind() == "class_declaration")
      .expect("class should exist");
    let node_match = item
      .match_node(&class_node)
      .expect("class should match item rule");
    let outline = item.extract(&node_match, vec![]);

    assert_eq!(outline.entry.name.as_ref(), "Foo");
    assert!(!outline.is_exported);
  }

  #[test]
  fn parses_outline_member_rule() {
    let rule = parse_rule(
      r#"
id: ts-member
language: TypeScript
role: member
parentRuleIds: [ts-interface]
symbolType: field
rule:
  kind: property_signature
name: member
"#,
    );

    let SerializableOutlineRule::Member(member) = rule else {
      panic!("expected member rule");
    };
    let member =
      MemberExtractor::try_from(member, &Default::default(), OutlineEntryDetail::Signature)
        .expect("member rule should parse");

    assert_eq!(member.common.rule.id, "ts-member");
    assert_eq!(member.parent_rule_ids, vec!["ts-interface"]);
    assert!(matches!(member.is_public, OutlinePredicate::Literal(true)));
  }

  #[test]
  fn serializes_with_internal_role_tag() {
    let rule = SerializableOutlineRule::Item(SerializableItemRule {
      common: SerializableOutlineCommon {
        id: "ts-function".into(),
        language: SupportLang::TypeScript,
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
