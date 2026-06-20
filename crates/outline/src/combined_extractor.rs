//! Combined outline extraction.
//!
//! Outline extraction has two matching phases. Top-level item extractors are
//! matched during a file-wide AST traversal, so they are indexed by node kind in
//! one dense table. Member extractors are only valid after a specific item
//! extractor has matched; they are grouped by parent item extractor id and then
//! indexed sparsely by child node kind inside that parent-scoped group.
//!
//! Extraction uses tree-sitter cursor-backed AST traversals instead of
//! `find_all`. For each node, the node kind selects the small set of extractors
//! that can possibly match. When a node becomes an outline item, item traversal
//! skips that node's descendants and member extraction runs in the matched
//! item's scope.

use ast_grep_config::GlobalRules;
use ast_grep_core::{
  Doc, Language, Matcher, Node, NodeMatch,
  meta_var::MetaVarEnv,
  tree_sitter::{LanguageExt, StrDoc, Visitor},
};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::extractor::{ItemExtractor, MemberExtractor, OutlineRuleError, SerializableOutlineRule};
use crate::model::{OutlineItem, OutlineMember};
use crate::options::OutlineExtractorOptions;

/// Runtime outline extractors organized for a shared item traversal.
pub struct CombinedExtractors<L: Language> {
  /// Top-level item extractors matched during the file-wide AST traversal.
  item_extractors: Vec<ItemExtractor<L>>,
  /// Dense node-kind index into `item_extractors`; shared across the whole file.
  item_kind_mapping: Vec<Vec<usize>>,
  /// Member extractors parsed once and referenced by parent-scoped groups below.
  member_extractors: Vec<MemberExtractor<L>>,
  /// Parent item extractor id to member extractors that may run inside it.
  member_mapping: HashMap<String, CombinedMemberExtractorGroup>,
  /// Runtime filters and detail level requested by the caller.
  options: OutlineExtractorOptions,
}

pub struct CombinedMemberExtractors<'a, L: Language> {
  /// Shared member extractor storage owned by `CombinedExtractors`.
  extractors: &'a [MemberExtractor<L>],
  /// Parent-scoped index that selects members relevant to one matched item rule.
  group: &'a CombinedMemberExtractorGroup,
}

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
    Self::try_from_rules(extractors, OutlineExtractorOptions::default(), globals)
  }

  pub fn try_from_rules(
    extractors: Vec<SerializableOutlineRule<L>>,
    options: OutlineExtractorOptions,
    globals: &GlobalRules,
  ) -> Result<Self, OutlineRuleError> {
    let mut item_extractors = Vec::with_capacity(extractors.len());
    let mut member_extractors = Vec::with_capacity(extractors.len());
    // NB: if member option is None, we won't pass any member extractors
    // so this is safe to fallback to default as we won't use it
    let member_options = options.members.clone().unwrap_or_default();
    for extractor in extractors {
      if !options.retain_rule(&extractor) {
        continue;
      }
      match extractor {
        SerializableOutlineRule::Item(item) => {
          item_extractors.push(ItemExtractor::try_from(item, globals, options.detail)?);
        }
        SerializableOutlineRule::Member(member) => {
          member_extractors.push(MemberExtractor::try_from(
            member,
            globals,
            member_options.detail,
          )?);
        }
      }
    }
    Ok(Self::new_with_options(
      item_extractors,
      member_extractors,
      options,
    ))
  }

  pub fn new(
    item_extractors: Vec<ItemExtractor<L>>,
    member_extractors: Vec<MemberExtractor<L>>,
  ) -> Self {
    Self::new_with_options(
      item_extractors,
      member_extractors,
      OutlineExtractorOptions::default(),
    )
  }

  pub fn new_with_options(
    item_extractors: Vec<ItemExtractor<L>>,
    member_extractors: Vec<MemberExtractor<L>>,
    options: OutlineExtractorOptions,
  ) -> Self {
    let item_kind_mapping = item_kind_mapping(&item_extractors);
    let member_mapping = member_mapping(&member_extractors);
    Self {
      item_extractors,
      item_kind_mapping,
      member_extractors,
      member_mapping,
      options,
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

  pub fn item_extractors_for_kind(&self, kind: u16) -> impl Iterator<Item = &ItemExtractor<L>> {
    self
      .item_kind_mapping
      .get(kind as usize)
      .into_iter()
      .flat_map(|indices| indices.iter().map(|&idx| &self.item_extractors[idx]))
  }

  pub fn extract<'tree>(&self, root: Node<'tree, StrDoc<L>>) -> Vec<OutlineItem<'tree>>
  where
    L: LanguageExt,
  {
    let mut items = vec![];
    let matcher = ItemTraversalMatcher { combined: self };
    for matched in Visitor::new(matcher).reentrant(false).visit(root) {
      if let Some((extractor, node_match)) = self.match_item(matched.get_node()) {
        let mut members = self.extract_members(extractor, node_match.get_node());
        members.retain(|member| self.options.keep_member(member));
        let item = extractor.extract(&node_match, members);
        if self.options.keep_item(&item) {
          items.push(item);
        }
      }
    }
    items
  }

  fn match_item<'tree>(
    &self,
    node: &Node<'tree, StrDoc<L>>,
  ) -> Option<(&ItemExtractor<L>, NodeMatch<'tree, StrDoc<L>>)>
  where
    L: LanguageExt,
  {
    self
      .item_extractors_for_kind(node.kind_id())
      .find_map(|extractor| {
        extractor
          .match_node(node)
          .map(|matched| (extractor, matched))
      })
  }

  fn extract_members<'tree>(
    &self,
    item_extractor: &ItemExtractor<L>,
    item_node: &Node<'tree, StrDoc<L>>,
  ) -> Vec<OutlineMember<'tree>>
  where
    L: LanguageExt,
  {
    let Some(member_extractors) = self.member_extractors_for(&item_extractor.common.rule.id) else {
      return vec![];
    };
    member_extractors.extract(item_node)
  }
}

impl<'a, L: Language> CombinedMemberExtractors<'a, L> {
  pub fn extractors_for_kind(&self, kind: u16) -> impl Iterator<Item = &MemberExtractor<L>> {
    self
      .group
      .kind_mapping
      .get(&kind)
      .into_iter()
      .flat_map(|indices| indices.iter().map(|&idx| &self.extractors[idx]))
  }

  pub fn extract<'tree>(&self, item_node: &Node<'tree, StrDoc<L>>) -> Vec<OutlineMember<'tree>>
  where
    L: LanguageExt,
  {
    let mut members = vec![];
    let matcher = MemberTraversalMatcher { combined: self };
    for child in item_node.children() {
      for matched in Visitor::new(&matcher).reentrant(false).visit(child) {
        if let Some(member) = self.extract_member(matched.get_node()) {
          members.push(member);
        }
      }
    }
    members
  }

  fn extract_member<'tree>(&self, node: &Node<'tree, StrDoc<L>>) -> Option<OutlineMember<'tree>>
  where
    L: LanguageExt,
  {
    self
      .extractors_for_kind(node.kind_id())
      .find_map(|extractor| extractor.match_node(node).map(|m| extractor.extract(&m)))
  }
}

struct ItemTraversalMatcher<'a, L: Language> {
  combined: &'a CombinedExtractors<L>,
}

impl<L: Language> Matcher for ItemTraversalMatcher<'_, L> {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    for extractor in self.combined.item_extractors_for_kind(node.kind_id()) {
      let Some(matched) = extractor.match_node(&node) else {
        continue;
      };
      *env = Cow::Owned(matched.get_env().clone());
      return Some(matched.get_node().clone());
    }
    None
  }
}

struct MemberTraversalMatcher<'a, L: Language> {
  combined: &'a CombinedMemberExtractors<'a, L>,
}

impl<L: Language> Matcher for MemberTraversalMatcher<'_, L> {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    for extractor in self.combined.extractors_for_kind(node.kind_id()) {
      let Some(matched) = extractor.match_node(&node) else {
        continue;
      };
      *env = Cow::Owned(matched.get_env().clone());
      return Some(matched.get_node().clone());
    }
    None
  }
}

fn item_kind_mapping<L: Language>(item_extractors: &[ItemExtractor<L>]) -> Vec<Vec<usize>> {
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
  member_extractors: &[MemberExtractor<L>],
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
  use crate::options::{OutlineEntryDetail, OutlineExtractorOptions, OutlineFlagFilter};
  use ast_grep_core::tree_sitter::LanguageExt;
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

  #[test]
  fn extracts_items_without_visiting_matched_item_descendants() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() { $$$BODY }
name: $NAME
"#,
    )
    .expect("extractors should deserialize");
    let combined = CombinedExtractors::try_from(extractors, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep(
      r#"
function outer() {
  function inner() {}
}
function after() {}
"#,
    );

    let items = combined.extract(grep.root());
    let names = items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(names, vec!["outer", "after"]);
  }

  #[test]
  fn extracts_members_only_from_matched_parent_items() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  pattern: class $NAME { $$$BODY }
name: $NAME
signature: class $NAME
---
id: ts-method
language: TypeScript
role: member
parentRuleIds: [ts-class]
symbolType: method
rule:
  pattern:
    context: class A { $NAME() { $$$BODY } }
    selector: method_definition
name: $NAME
signature: $NAME()
"#,
    )
    .expect("extractors should deserialize");
    let combined = CombinedExtractors::try_from(extractors, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep(
      r#"
class Box {
  parse() {
    function local() {}
  }
}
function standalone() {}
"#,
    );

    let items = combined.extract(grep.root());

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].entry.name, "Box");
    assert_eq!(items[0].members.len(), 1);
    assert_eq!(items[0].members[0].entry.name, "parse");
    assert_eq!(items[0].members[0].entry.signature, "parse()");
  }

  #[test]
  fn compile_options_disable_members_and_name_only_signatures() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  pattern: class $NAME { $$$BODY }
name: $NAME
signature: class $NAME
---
id: ts-method
language: TypeScript
role: member
parentRuleIds: [ts-class]
symbolType: method
rule:
  pattern:
    context: class A { $NAME() { $$$BODY } }
    selector: method_definition
name: $NAME
signature: $NAME()
"#,
    )
    .expect("extractors should deserialize");
    let options = OutlineExtractorOptions {
      members: None,
      detail: OutlineEntryDetail::Name,
      ..Default::default()
    };
    let combined = CombinedExtractors::try_from_rules(extractors, options, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep("class Box { parse() {} }");

    let items = combined.extract(grep.root());

    assert!(combined.member_extractors.is_empty());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].entry.name, "Box");
    assert!(items[0].entry.signature.is_empty());
    assert!(items[0].members.is_empty());
  }

  #[test]
  fn compile_options_filter_rules_and_runtime_flags() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-import
language: TypeScript
role: item
symbolType: module
rule:
  kind: import_statement
name: import
isImport: true
isExported: false
---
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() { $$$BODY }
name: $NAME
isImport: false
"#,
    )
    .expect("extractors should deserialize");
    let options = OutlineExtractorOptions {
      imports: OutlineFlagFilter::Yes,
      ..Default::default()
    };
    let combined = CombinedExtractors::try_from_rules(extractors, options, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep(
      r#"
import { readFile } from 'node:fs';
function local() {}
"#,
    );

    let items = combined.extract(grep.root());

    assert_eq!(combined.item_extractors.len(), 1);
    assert_eq!(combined.item_extractors[0].common.rule.id, "ts-import");
    assert_eq!(items.len(), 1);
    assert!(items[0].is_import);
  }
}
