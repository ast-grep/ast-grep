//! Combined outline extraction.
//!
//! Outline extraction has two matching phases. Top-level item extractors are
//! matched during a file-wide AST traversal, so they are indexed by node kind in
//! one dense table. Member extractors are only valid after a specific item
//! extractor has matched; they are grouped by parent item extractor id and then
//! indexed sparsely by child node kind inside that parent-scoped group.
//!
//! Extraction uses a single tree-sitter cursor-backed traversal instead of
//! `find_all` or a second member pass per matched item. The traversal has two
//! states: at file scope it matches item extractors; inside a matched item it
//! switches to the item's scoped member extractors until the cursor leaves that
//! item range.

use ast_grep_config::GlobalRules;
use ast_grep_core::{
  Language, Matcher, Node, NodeMatch,
  tree_sitter::{
    LanguageExt, StrDoc,
    traversal::{Prune, PruneSubtree},
  },
};
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
      .item_indices_for_kind(kind)
      .iter()
      .map(|&idx| &self.item_extractors[idx])
  }

  fn item_indices_for_kind(&self, kind: u16) -> &[usize] {
    indices_for_kind(&self.item_kind_mapping, kind)
  }

  pub fn extract<'tree>(&self, root: Node<'tree, StrDoc<L>>) -> Vec<OutlineItem<'tree>>
  where
    L: LanguageExt,
  {
    CombinedTraversal::new(self, root).extract()
  }

  fn match_item<'tree>(
    &self,
    node: &Node<'tree, StrDoc<L>>,
  ) -> Option<(&ItemExtractor<L>, NodeMatch<'tree, StrDoc<L>>)>
  where
    L: LanguageExt,
  {
    for &idx in self.item_indices_for_kind(node.kind_id()) {
      let extractor = &self.item_extractors[idx];
      if let Some(matched) = extractor.match_node(node) {
        return Some((extractor, matched));
      }
    }
    None
  }
}

impl<'a, L: Language> CombinedMemberExtractors<'a, L> {
  pub fn extractors_for_kind(&self, kind: u16) -> impl Iterator<Item = &MemberExtractor<L>> {
    self
      .indices_for_kind(kind)
      .iter()
      .map(|&idx| &self.extractors[idx])
  }

  fn indices_for_kind(&self, kind: u16) -> &[usize] {
    self
      .group
      .kind_mapping
      .get(&kind)
      .map(Vec::as_slice)
      .unwrap_or(&[])
  }

  fn extract_member<'tree>(&self, node: &Node<'tree, StrDoc<L>>) -> Option<OutlineMember<'tree>>
  where
    L: LanguageExt,
  {
    for &idx in self.indices_for_kind(node.kind_id()) {
      let extractor = &self.extractors[idx];
      if let Some(matched) = extractor.match_node(node) {
        return Some(extractor.extract(matched));
      }
    }
    None
  }
}

struct CombinedTraversal<'a, 'tree, L: LanguageExt> {
  combined: &'a CombinedExtractors<L>,
  traversal: Prune<'tree, L>,
  state: TraversalState<'a, 'tree, L>,
  items: Vec<OutlineItem<'tree>>,
}

// Keep the active member scope inline. The traverser owns exactly one state,
// and boxing it would add one allocation for every item that can contain members.
#[allow(clippy::large_enum_variant)]
enum TraversalState<'a, 'tree, L: LanguageExt> {
  /// File scope: match candidate nodes against item extractors.
  Items,
  /// Parent item scope: match descendants against only that item's member rules.
  Members(MemberScope<'a, 'tree, L>),
}

struct MemberScope<'a, 'tree, L: LanguageExt> {
  extractor: &'a ItemExtractor<L>,
  node_match: NodeMatch<'tree, StrDoc<L>>,
  member_extractors: CombinedMemberExtractors<'a, L>,
  members: Vec<OutlineMember<'tree>>,
  item_subtree: PruneSubtree<'tree>,
}

impl<'a, 'tree, L: LanguageExt> CombinedTraversal<'a, 'tree, L> {
  fn new(combined: &'a CombinedExtractors<L>, root: Node<'tree, StrDoc<L>>) -> Self {
    Self {
      combined,
      traversal: Prune::new(&root),
      state: TraversalState::Items,
      items: vec![],
    }
  }

  fn extract(mut self) -> Vec<OutlineItem<'tree>> {
    while let Some(node) = self.traversal.current_node() {
      if self.finished_member_scope() {
        self.finish_member_scope();
        continue;
      }
      match &mut self.state {
        TraversalState::Items => self.visit_item(node),
        TraversalState::Members(scope) => {
          Self::visit_member(scope, node, &mut self.traversal, &self.combined.options);
        }
      }
    }
    self.finish_member_scope();
    self.items
  }

  fn visit_item(&mut self, node: Node<'tree, StrDoc<L>>) {
    let item_subtree = self.traversal.current_subtree();
    let Some((extractor, node_match)) = self.combined.match_item(&node) else {
      self.traversal.descend();
      return;
    };
    let Some(member_extractors) = self
      .combined
      .member_extractors_for(&extractor.common.rule.id)
    else {
      self.push_item(extractor.extract(node_match, vec![]));
      self.traversal.skip_subtree();
      return;
    };
    self.state = TraversalState::Members(MemberScope::new(
      extractor,
      node_match,
      member_extractors,
      item_subtree,
    ));
    self.traversal.descend();
  }

  fn visit_member(
    scope: &mut MemberScope<'a, 'tree, L>,
    node: Node<'tree, StrDoc<L>>,
    traversal: &mut Prune<'tree, L>,
    options: &OutlineExtractorOptions,
  ) {
    if let Some(member) = scope.member_extractors.extract_member(&node) {
      if options.keep_member(&member) {
        scope.members.push(member);
      }
      traversal.skip_subtree();
    } else {
      traversal.descend();
    }
  }

  fn finished_member_scope(&self) -> bool {
    match &self.state {
      TraversalState::Items => false,
      TraversalState::Members(scope) => self.traversal.has_left_subtree(scope.item_subtree),
    }
  }

  fn finish_member_scope(&mut self) {
    let state = std::mem::replace(&mut self.state, TraversalState::Items);
    if let TraversalState::Members(scope) = state {
      self.push_item(scope.into_item());
    }
  }

  fn push_item(&mut self, item: OutlineItem<'tree>) {
    if self.combined.options.keep_item(&item) {
      self.items.push(item);
    }
  }
}

impl<'a, 'tree, L: LanguageExt> MemberScope<'a, 'tree, L> {
  fn new(
    extractor: &'a ItemExtractor<L>,
    node_match: NodeMatch<'tree, StrDoc<L>>,
    member_extractors: CombinedMemberExtractors<'a, L>,
    item_subtree: PruneSubtree<'tree>,
  ) -> Self {
    Self {
      extractor,
      node_match,
      member_extractors,
      members: vec![],
      item_subtree,
    }
  }

  fn into_item(self) -> OutlineItem<'tree> {
    self.extractor.extract(self.node_match, self.members)
  }
}

fn indices_for_kind(mapping: &[Vec<usize>], kind: u16) -> &[usize] {
  mapping.get(kind as usize).map(Vec::as_slice).unwrap_or(&[])
}

fn push_kind_mapping(mapping: &mut Vec<Vec<usize>>, kind: usize, idx: usize) {
  while mapping.len() <= kind {
    mapping.push(vec![]);
  }
  mapping[kind].push(idx);
}

fn item_kind_mapping<L: Language>(item_extractors: &[ItemExtractor<L>]) -> Vec<Vec<usize>> {
  let mut mapping = Vec::new();
  for (idx, extractor) in item_extractors.iter().enumerate() {
    let Some(kinds) = extractor.common.rule.matcher.potential_kinds() else {
      continue;
    };
    for kind in &kinds {
      push_kind_mapping(&mut mapping, kind, idx);
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
  fn resumes_item_matching_after_member_scope() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  pattern: class $NAME { $$$BODY }
name: $NAME
---
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() { $$$BODY }
name: $NAME
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
"#,
    )
    .expect("extractors should deserialize");
    let combined = CombinedExtractors::try_from(extractors, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep(
      r#"
class Box {
  parse() {}
}
function after() {}
"#,
    );

    let items = combined.extract(grep.root());

    let names = items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(names, vec!["Box", "after"]);
    assert_eq!(items[0].members.len(), 1);
    assert_eq!(items[0].members[0].entry.name, "parse");
    assert!(items[1].members.is_empty());
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
