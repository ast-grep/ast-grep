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

use crate::extractor::{
  ItemExtractor, MemberExtractor, OutlineRuleError, OutlineStopBy, SerializableOutlineRule,
};
use crate::model::{OutlineItem, OutlineMember};
use crate::options::OutlineExtractorOptions;

const POTENTIAL_KINDS_INVARIANT: &str =
  "compiled outline rules must have potential kinds because RuleConfig rejects unconstrained rules";

/// Runtime outline extractors organized for a shared item traversal.
pub struct CombinedExtractors<L: Language> {
  /// Top-level item extractors matched during the file-wide AST traversal.
  item_extractors: Vec<ItemExtractor<L>>,
  /// Dense node-kind index into `item_extractors`; shared across the whole file.
  item_kind_index: Vec<Vec<usize>>,
  /// Whether every retained item extractor stops at direct children.
  all_items_immediate: bool,
  /// Member extractors parsed once and referenced by parent-scoped groups below.
  member_extractors: Vec<MemberExtractor<L>>,
  /// Parent item extractor id to member extractors that may run inside it.
  member_index_by_parent: HashMap<String, MemberExtractorIndex>,
  /// Runtime filters and detail level requested by the caller.
  options: OutlineExtractorOptions,
}

struct ScopedMemberExtractors<'a, L: Language> {
  /// Shared member extractor storage owned by `CombinedExtractors`.
  extractors: &'a [MemberExtractor<L>],
  /// Parent-scoped index that selects members relevant to one matched item rule.
  index: &'a MemberExtractorIndex,
}

#[derive(Default)]
struct MemberExtractorIndex {
  /// Sparse node-kind index into `member_extractors` for scoped member traversal.
  kind_mapping: HashMap<u16, Vec<usize>>,
  /// Whether this parent scope contains an extractor that traverses to the end.
  has_end: bool,
}

#[derive(Default)]
struct TraversalVisitCounter {
  #[cfg(test)]
  count: usize,
}

impl TraversalVisitCounter {
  #[inline]
  fn record(&mut self) {
    #[cfg(test)]
    {
      self.count += 1;
    }
  }
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
    validate_parent_rule_ids(&extractors)?;
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

  fn new_with_options(
    item_extractors: Vec<ItemExtractor<L>>,
    member_extractors: Vec<MemberExtractor<L>>,
    options: OutlineExtractorOptions,
  ) -> Self {
    let item_kind_index = item_kind_index(&item_extractors);
    let all_items_immediate = item_extractors
      .iter()
      .all(|extractor| extractor.common.stop_by == OutlineStopBy::Immediate);
    let member_index_by_parent = member_index_by_parent(&member_extractors);
    Self {
      item_extractors,
      item_kind_index,
      all_items_immediate,
      member_extractors,
      member_index_by_parent,
      options,
    }
  }

  fn member_scope_for(&self, parent_id: &str) -> Option<ScopedMemberExtractors<'_, L>> {
    self
      .member_index_by_parent
      .get(parent_id)
      .map(|index| ScopedMemberExtractors {
        extractors: &self.member_extractors,
        index,
      })
  }

  fn item_extractors_for_kind(
    &self,
    kind: u16,
    allow_immediate: bool,
  ) -> impl Iterator<Item = &ItemExtractor<L>> {
    self
      .item_kind_index
      .get(kind as usize)
      .map(Vec::as_slice)
      .unwrap_or(&[])
      .iter()
      .map(|&idx| &self.item_extractors[idx])
      .filter(move |extractor| allow_immediate || extractor.common.stop_by == OutlineStopBy::End)
  }

  pub fn extract<'a, 'tree>(
    &'a self,
    root: Node<'tree, StrDoc<L>>,
  ) -> impl Iterator<Item = OutlineItem<'tree>> + use<'a, 'tree, L>
  where
    L: LanguageExt,
  {
    self.item_iter(root)
  }

  fn item_iter<'a, 'tree>(&'a self, root: Node<'tree, StrDoc<L>>) -> OutlineItemIter<'a, 'tree, L>
  where
    L: LanguageExt,
  {
    OutlineItemIter {
      combined: self,
      traversal: Prune::new(&root),
      at_source: true,
      descendant_subtree: None,
      visit_counter: TraversalVisitCounter::default(),
    }
  }

  #[cfg(test)]
  fn extract_with_visit_count<'tree>(
    &self,
    root: Node<'tree, StrDoc<L>>,
  ) -> (Vec<OutlineItem<'tree>>, usize)
  where
    L: LanguageExt,
  {
    let mut iter = self.item_iter(root);
    let items = iter.by_ref().collect();
    (items, iter.visit_counter.count)
  }

  fn match_item<'tree>(
    &self,
    node: &Node<'tree, StrDoc<L>>,
    allow_immediate: bool,
  ) -> Option<(&ItemExtractor<L>, NodeMatch<'tree, StrDoc<L>>)>
  where
    L: LanguageExt,
  {
    for extractor in self.item_extractors_for_kind(node.kind_id(), allow_immediate) {
      if let Some(matched) = extractor.match_node(node) {
        return Some((extractor, matched));
      }
    }
    None
  }
}

impl<'a, L: Language> ScopedMemberExtractors<'a, L> {
  fn extractors_for_kind(
    &self,
    kind: u16,
    allow_immediate: bool,
  ) -> impl Iterator<Item = &MemberExtractor<L>> {
    self
      .index
      .kind_mapping
      .get(&kind)
      .map(Vec::as_slice)
      .unwrap_or(&[])
      .iter()
      .map(|&idx| &self.extractors[idx])
      .filter(move |extractor| allow_immediate || extractor.common.stop_by == OutlineStopBy::End)
  }

  fn extract_member<'tree>(
    &self,
    node: &Node<'tree, StrDoc<L>>,
    allow_immediate: bool,
  ) -> Option<OutlineMember<'tree>>
  where
    L: LanguageExt,
  {
    for extractor in self.extractors_for_kind(node.kind_id(), allow_immediate) {
      if let Some(matched) = extractor.match_node(node) {
        return Some(extractor.extract(&matched));
      }
    }
    None
  }

  fn all_immediate(&self) -> bool {
    !self.index.has_end
  }
}

struct OutlineItemIter<'a, 'tree, L: LanguageExt> {
  combined: &'a CombinedExtractors<L>,
  traversal: Prune<'tree, L>,
  at_source: bool,
  descendant_subtree: Option<PruneSubtree<'tree>>,
  visit_counter: TraversalVisitCounter,
}

impl<'a, 'tree, L: LanguageExt> Iterator for OutlineItemIter<'a, 'tree, L> {
  type Item = OutlineItem<'tree>;

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let node = self.traversal.current_node()?;
      if let Some(item) = self.visit_current_node(node) {
        return Some(item);
      }
    }
  }
}

impl<'a, 'tree, L: LanguageExt> OutlineItemIter<'a, 'tree, L> {
  fn visit_current_node(&mut self, node: Node<'tree, StrDoc<L>>) -> Option<OutlineItem<'tree>> {
    if self
      .descendant_subtree
      .is_some_and(|subtree| self.traversal.has_left_subtree(subtree))
    {
      self.descendant_subtree = None;
    }
    self.visit_counter.record();
    let is_direct_child = !self.at_source && self.descendant_subtree.is_none();
    let combined = self.combined;
    let item_subtree = self.traversal.current_subtree();
    let Some((extractor, node_match)) = combined.match_item(&node, is_direct_child) else {
      self.advance_after_no_match(is_direct_child, item_subtree);
      return None;
    };
    let members = self.collect_members_for_item(&extractor.common.rule.id, item_subtree);
    let item = extractor.extract(&node_match, members);
    combined.options.keep_item(&item).then_some(item)
  }

  fn advance_after_no_match(
    &mut self,
    is_direct_child: bool,
    current_subtree: PruneSubtree<'tree>,
  ) {
    if self.at_source {
      self.at_source = false;
      self.traversal.descend();
    } else if is_direct_child {
      if self.combined.all_items_immediate {
        self.traversal.skip_subtree();
      } else {
        self.traversal.descend();
        self.descendant_subtree = Some(current_subtree);
      }
    } else {
      self.traversal.descend();
    }
  }

  fn collect_members_for_item(
    &mut self,
    item_rule_id: &str,
    item_subtree: PruneSubtree<'tree>,
  ) -> Vec<OutlineMember<'tree>> {
    let Some(member_extractors) = self.combined.member_scope_for(item_rule_id) else {
      self.traversal.skip_subtree();
      return vec![];
    };
    self.traversal.descend();
    collect_scoped_members(
      &mut self.traversal,
      member_extractors,
      &self.combined.options,
      item_subtree,
      &mut self.visit_counter,
    )
  }
}

fn validate_parent_rule_ids<L>(
  extractors: &[SerializableOutlineRule<L>],
) -> Result<(), OutlineRuleError> {
  let mut rule_roles = HashMap::new();
  for extractor in extractors {
    rule_roles.insert(
      extractor.common().id.as_str(),
      matches!(extractor, SerializableOutlineRule::Item(_)),
    );
  }
  for extractor in extractors {
    let SerializableOutlineRule::Member(member) = extractor else {
      continue;
    };
    for parent_id in &member.parent_rule_ids {
      match rule_roles.get(parent_id.as_str()) {
        Some(true) => {}
        Some(false) => {
          return Err(OutlineRuleError::InvalidParentRuleRole {
            rule_id: member.common.id.clone(),
            parent_id: parent_id.clone(),
          });
        }
        None => {
          return Err(OutlineRuleError::UnknownParentRuleId {
            rule_id: member.common.id.clone(),
            parent_id: parent_id.clone(),
          });
        }
      }
    }
  }
  Ok(())
}

fn collect_scoped_members<'a, 'tree, L: LanguageExt>(
  traversal: &mut Prune<'tree, L>,
  member_extractors: ScopedMemberExtractors<'a, L>,
  options: &OutlineExtractorOptions,
  item_subtree: PruneSubtree<'tree>,
  visit_counter: &mut TraversalVisitCounter,
) -> Vec<OutlineMember<'tree>> {
  let mut members = vec![];
  let mut descendant_subtree = None;
  while let Some(node) = traversal.current_node() {
    if traversal.has_left_subtree(item_subtree) {
      break;
    }
    if descendant_subtree.is_some_and(|subtree| traversal.has_left_subtree(subtree)) {
      descendant_subtree = None;
    }
    visit_counter.record();
    let is_direct_child = descendant_subtree.is_none();
    if let Some(member) = member_extractors.extract_member(&node, is_direct_child) {
      if options.keep_member(&member) {
        members.push(member);
      }
      traversal.skip_subtree();
    } else if is_direct_child && member_extractors.all_immediate() {
      traversal.skip_subtree();
    } else if is_direct_child {
      let current_subtree = traversal.current_subtree();
      traversal.descend();
      descendant_subtree = Some(current_subtree);
    } else {
      traversal.descend();
    }
  }
  members
}

fn push_kind_mapping(mapping: &mut Vec<Vec<usize>>, kind: usize, idx: usize) {
  while mapping.len() <= kind {
    mapping.push(vec![]);
  }
  mapping[kind].push(idx);
}

fn item_kind_index<L: Language>(item_extractors: &[ItemExtractor<L>]) -> Vec<Vec<usize>> {
  let mut mapping = Vec::new();
  for (idx, extractor) in item_extractors.iter().enumerate() {
    let kinds = extractor
      .common
      .rule
      .matcher
      .potential_kinds()
      .expect(POTENTIAL_KINDS_INVARIANT);
    for kind in &kinds {
      push_kind_mapping(&mut mapping, kind, idx);
    }
  }
  mapping
}

fn member_index_by_parent<L: Language>(
  member_extractors: &[MemberExtractor<L>],
) -> HashMap<String, MemberExtractorIndex> {
  let mut mapping: HashMap<String, MemberExtractorIndex> = HashMap::new();
  for (idx, extractor) in member_extractors.iter().enumerate() {
    for parent_id in &extractor.parent_rule_ids {
      let index = mapping.entry(parent_id.clone()).or_default();
      index.has_end |= extractor.common.stop_by == OutlineStopBy::End;
      let kinds = extractor
        .common
        .rule
        .matcher
        .potential_kinds()
        .expect(POTENTIAL_KINDS_INVARIANT);
      for kind in &kinds {
        index.kind_mapping.entry(kind as u16).or_default().push(idx);
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
  fn immediate_item_scope_prunes_unmatched_direct_child_subtrees() {
    let source = r#"if (ready) { console.log(ready); }
function direct() {}"#;
    let immediate_rules = parse_outline_rules::<SupportLang>(
      r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
stopBy: immediate
rule:
  kind: function_declaration
  has:
    field: name
    pattern: $NAME
name: $NAME
"#,
    )
    .expect("extractors should deserialize");
    let end_rules = parse_outline_rules::<SupportLang>(
      r#"
id: ts-function
language: TypeScript
role: item
symbolType: function
stopBy: end
rule:
  kind: function_declaration
  has:
    field: name
    pattern: $NAME
name: $NAME
"#,
    )
    .expect("extractors should deserialize");
    let immediate = CombinedExtractors::try_from(immediate_rules, &Default::default())
      .expect("extractors should parse");
    let end = CombinedExtractors::try_from(end_rules, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep(source);

    let (immediate_items, immediate_visits) = immediate.extract_with_visit_count(grep.root());
    let (end_items, end_visits) = end.extract_with_visit_count(grep.root());
    let immediate_names = immediate_items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();
    let end_names = end_items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(immediate_names, end_names);
    assert_eq!(immediate_names, vec!["direct"]);
    assert_eq!(immediate_visits, 3);
    assert!(end_visits > immediate_visits);
  }

  #[test]
  fn immediate_member_scope_prunes_unmatched_direct_child_subtrees() {
    let source = r#"
class Box {
  direct() {}
  field = { nested: { value: 1 } };
}
"#;
    let immediate_rules = parse_outline_rules::<SupportLang>(
      r#"
id: ts-class-body
language: TypeScript
role: item
symbolType: class
rule:
  kind: class_body
name: body
---
id: ts-method
language: TypeScript
role: member
parentRuleIds: [ts-class-body]
symbolType: method
stopBy: immediate
rule:
  kind: method_definition
  has:
    field: name
    pattern: $NAME
name: $NAME
"#,
    )
    .expect("extractors should deserialize");
    let end_rules = parse_outline_rules::<SupportLang>(
      r#"
id: ts-class-body
language: TypeScript
role: item
symbolType: class
rule:
  kind: class_body
name: body
---
id: ts-method
language: TypeScript
role: member
parentRuleIds: [ts-class-body]
symbolType: method
stopBy: end
rule:
  kind: method_definition
  has:
    field: name
    pattern: $NAME
name: $NAME
"#,
    )
    .expect("extractors should deserialize");
    let immediate = CombinedExtractors::try_from(immediate_rules, &Default::default())
      .expect("extractors should parse");
    let end = CombinedExtractors::try_from(end_rules, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::TypeScript.ast_grep(source);

    let (immediate_items, immediate_visits) = immediate.extract_with_visit_count(grep.root());
    let (end_items, end_visits) = end.extract_with_visit_count(grep.root());
    let immediate_members = immediate_items[0]
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    let end_members = end_items[0]
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(immediate_members, end_members);
    assert_eq!(immediate_members, vec!["direct"]);
    assert!(end_visits > immediate_visits);
  }

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
      .item_extractors_for_kind(function_kind, true)
      .collect::<Vec<_>>();
    let member_extractors = combined
      .member_scope_for("ts-function")
      .expect("member extractors should exist");
    let identifier_kind = SupportLang::TypeScript.kind_to_id("identifier");
    let identifier_members = member_extractors
      .extractors_for_kind(identifier_kind, true)
      .collect::<Vec<_>>();

    assert!(combined.member_scope_for("missing").is_none());
    assert_eq!(item_extractors.len(), 1);
    assert_eq!(item_extractors[0].common.rule.id, "ts-function");
    assert_eq!(identifier_members.len(), 1);
    assert_eq!(identifier_members[0].common.rule.id, "ts-member");
  }

  #[test]
  fn stop_by_partitions_use_only_rules_retained_by_options() {
    let item_extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-import
language: TypeScript
role: item
symbolType: module
stopBy: immediate
rule:
  kind: import_statement
name: import
isImport: true
---
id: ts-function
language: TypeScript
role: item
symbolType: function
stopBy: end
rule:
  kind: function_declaration
name: function
isImport: false
"#,
    )
    .expect("extractors should deserialize");
    let item_options = OutlineExtractorOptions {
      imports: OutlineFlagFilter::Yes,
      ..Default::default()
    };
    let combined =
      CombinedExtractors::try_from_rules(item_extractors, item_options, &Default::default())
        .expect("extractors should parse");

    assert_eq!(combined.item_extractors.len(), 1);
    assert!(combined.all_items_immediate);

    let member_extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  kind: class_declaration
name: class
---
id: ts-public-method
language: TypeScript
role: member
parentRuleIds: [ts-class]
symbolType: method
stopBy: immediate
rule:
  kind: method_definition
name: method
isPublic: true
---
id: ts-private-method
language: TypeScript
role: member
parentRuleIds: [ts-class]
symbolType: method
stopBy: end
rule:
  kind: method_definition
name: method
isPublic: false
"#,
    )
    .expect("extractors should deserialize");
    let member_options = OutlineExtractorOptions {
      members: Some(crate::options::OutlineMemberOptions {
        public: OutlineFlagFilter::Yes,
        ..Default::default()
      }),
      ..Default::default()
    };
    let combined =
      CombinedExtractors::try_from_rules(member_extractors, member_options, &Default::default())
        .expect("extractors should parse");
    let scope = combined
      .member_scope_for("ts-class")
      .expect("member scope should exist");

    assert_eq!(combined.member_extractors.len(), 1);
    assert!(scope.all_immediate());
  }

  #[test]
  fn rejects_unknown_member_parent_rule_id() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-member
language: TypeScript
role: member
parentRuleIds: [missing-parent]
symbolType: method
rule:
  kind: method_definition
name: member
"#,
    )
    .expect("extractors should deserialize");

    let Err(err) = CombinedExtractors::try_from(extractors, &Default::default()) else {
      panic!("unknown parent id should be rejected");
    };

    assert!(matches!(err, OutlineRuleError::UnknownParentRuleId { .. }));
    assert_eq!(
      err.to_string(),
      "Member rule `ts-member` references unknown parent rule `missing-parent`"
    );
  }

  #[test]
  fn rejects_member_parent_rule_id_that_points_to_member_rule() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: ts-parent-member
language: TypeScript
role: member
parentRuleIds: [ts-class]
symbolType: method
rule:
  kind: method_definition
name: parent
---
id: ts-member
language: TypeScript
role: member
parentRuleIds: [ts-parent-member]
symbolType: method
rule:
  kind: method_definition
name: child
---
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  pattern: class $NAME { $$$BODY }
name: $NAME
"#,
    )
    .expect("extractors should deserialize");

    let Err(err) = CombinedExtractors::try_from(extractors, &Default::default()) else {
      panic!("member parent ids should only reference item rules");
    };

    assert!(matches!(
      err,
      OutlineRuleError::InvalidParentRuleRole { .. }
    ));
    assert_eq!(
      err.to_string(),
      "Member rule `ts-member` cannot use member rule `ts-parent-member` as a parent"
    );
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

    let items = combined.extract(grep.root()).collect::<Vec<_>>();
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

    let items = combined.extract(grep.root()).collect::<Vec<_>>();

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

    let items = combined.extract(grep.root()).collect::<Vec<_>>();

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

    let items = combined.extract(grep.root()).collect::<Vec<_>>();

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

    let items = combined.extract(grep.root()).collect::<Vec<_>>();

    assert_eq!(combined.item_extractors.len(), 1);
    assert_eq!(combined.item_extractors[0].common.rule.id, "ts-import");
    assert_eq!(items.len(), 1);
    assert!(items[0].is_import);
  }
}
