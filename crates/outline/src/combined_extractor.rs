//! Combined outline extractor indexes.
//!
//! Outline extraction has two matching phases. Top-level item extractors are
//! matched during a file-wide AST traversal, so they are indexed by node kind in
//! one dense table. Member extractors are only valid after a specific item
//! extractor has matched; they are grouped by parent item extractor id and then
//! indexed sparsely by child node kind inside that parent-scoped group.
//!
//! This module only builds those indexes. It does not traverse the AST or
//! perform scoped member extraction.

use ast_grep_config::GlobalRules;
use ast_grep_core::{Language, Matcher};
use std::collections::HashMap;

use crate::extractor::{
  OutlineItemRule, OutlineMemberRule, OutlineRuleError, SerializableOutlineRule,
};

/// Runtime outline extractors organized for a shared item traversal.
#[allow(dead_code)]
pub struct CombinedExtractors<L: Language> {
  /// Top-level item extractors matched during the file-wide AST traversal.
  item_extractors: Vec<OutlineItemRule<L>>,
  /// Dense node-kind index into `item_extractors`; shared across the whole file.
  item_kind_mapping: Vec<Vec<usize>>,
  /// Member extractors parsed once and referenced by parent-scoped groups below.
  member_extractors: Vec<OutlineMemberRule<L>>,
  /// Parent item extractor id to member extractors that may run inside it.
  member_mapping: HashMap<String, CombinedMemberExtractorGroup>,
}

#[allow(dead_code)]
pub struct CombinedMemberExtractors<'a, L: Language> {
  /// Shared member extractor storage owned by `CombinedExtractors`.
  extractors: &'a [OutlineMemberRule<L>],
  /// Parent-scoped index that selects members relevant to one matched item rule.
  group: &'a CombinedMemberExtractorGroup,
}

#[allow(dead_code)]
#[derive(Default)]
struct CombinedMemberExtractorGroup {
  /// Sparse node-kind index into `member_extractors` for scoped member traversal.
  kind_mapping: HashMap<u16, Vec<usize>>,
}

impl<L: Language> CombinedExtractors<L> {
  pub fn try_from(
    extractors: Vec<SerializableOutlineRule<L>>,
    globals: &GlobalRules,
  ) -> Result<Self, OutlineRuleError> {
    let mut item_extractors = vec![];
    let mut member_extractors = vec![];
    for extractor in extractors {
      match extractor {
        SerializableOutlineRule::Item(item) => {
          item_extractors.push(OutlineItemRule::try_from(item, globals)?);
        }
        SerializableOutlineRule::Member(member) => {
          member_extractors.push(OutlineMemberRule::try_from(member, globals)?);
        }
      }
    }
    Ok(Self::new(item_extractors, member_extractors))
  }

  pub fn new(
    item_extractors: Vec<OutlineItemRule<L>>,
    member_extractors: Vec<OutlineMemberRule<L>>,
  ) -> Self {
    let item_kind_mapping = item_kind_mapping(&item_extractors);
    let member_mapping = member_mapping(&member_extractors);
    Self {
      item_extractors,
      item_kind_mapping,
      member_extractors,
      member_mapping,
    }
  }

  pub fn member_extractors_for(&self, parent_id: &str) -> Option<CombinedMemberExtractors<'_, L>> {
    self
      .member_mapping
      .get(parent_id)
      .map(|group| CombinedMemberExtractors {
        extractors: &self.member_extractors,
        group,
      })
  }

  pub fn item_extractors_for_kind(&self, kind: u16) -> impl Iterator<Item = &OutlineItemRule<L>> {
    self
      .item_kind_mapping
      .get(kind as usize)
      .into_iter()
      .flat_map(|indices| indices.iter().map(|&idx| &self.item_extractors[idx]))
  }
}

impl<'a, L: Language> CombinedMemberExtractors<'a, L> {
  pub fn extractors_for_kind(&self, kind: u16) -> impl Iterator<Item = &OutlineMemberRule<L>> {
    self
      .group
      .kind_mapping
      .get(&kind)
      .into_iter()
      .flat_map(|indices| indices.iter().map(|&idx| &self.extractors[idx]))
  }
}

fn item_kind_mapping<L: Language>(item_extractors: &[OutlineItemRule<L>]) -> Vec<Vec<usize>> {
  let mut mapping = Vec::new();
  for (idx, extractor) in item_extractors.iter().enumerate() {
    let Some(kinds) = extractor.common.rule.matcher.potential_kinds() else {
      continue;
    };
    for kind in &kinds {
      while mapping.len() <= kind {
        mapping.push(vec![]);
      }
      mapping[kind].push(idx);
    }
  }
  mapping
}

fn member_mapping<L: Language>(
  member_extractors: &[OutlineMemberRule<L>],
) -> HashMap<String, CombinedMemberExtractorGroup> {
  let mut mapping: HashMap<String, CombinedMemberExtractorGroup> = HashMap::new();
  for (idx, extractor) in member_extractors.iter().enumerate() {
    for parent_id in &extractor.parent_rule_ids {
      let group = mapping.entry(parent_id.clone()).or_default();
      let Some(kinds) = extractor.common.rule.matcher.potential_kinds() else {
        continue;
      };
      for kind in &kinds {
        group.kind_mapping.entry(kind as u16).or_default().push(idx);
      }
    }
  }
  mapping
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::extractor::parse_outline_rules;
  use ast_grep_language::SupportLang;

  #[test]
  fn combines_extractors_by_item_kind_and_parent_id() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() { $$$BODY }
name: $NAME
---
id: ts-member
language: TypeScript
role: member
parentRuleIds: [ts-function]
symbolType: field
rule:
  kind: identifier
name: member
---
id: ts-other-member
language: TypeScript
role: member
parentRuleIds: [ts-function]
symbolType: field
rule:
  kind: property_signature
name: other
"#,
    )
    .expect("extractors should deserialize");

    let combined = CombinedExtractors::try_from(extractors, &Default::default())
      .expect("extractors should parse");
    let function_kind = SupportLang::TypeScript.kind_to_id("function_declaration");
    let item_extractors = combined
      .item_extractors_for_kind(function_kind)
      .collect::<Vec<_>>();
    let member_extractors = combined
      .member_extractors_for("ts-function")
      .expect("member extractors should exist");
    let identifier_kind = SupportLang::TypeScript.kind_to_id("identifier");
    let identifier_members = member_extractors
      .extractors_for_kind(identifier_kind)
      .collect::<Vec<_>>();

    assert!(combined.member_extractors_for("missing").is_none());
    assert_eq!(item_extractors.len(), 1);
    assert_eq!(item_extractors[0].common.rule.id, "ts-function");
    assert_eq!(identifier_members.len(), 1);
    assert_eq!(identifier_members[0].common.rule.id, "ts-member");
  }
}
