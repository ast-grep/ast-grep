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
use std::collections::{HashMap, HashSet};

use crate::extractor::{ItemExtractor, MemberExtractor, OutlineRuleError, SerializableOutlineRule};
use crate::model::{OutlineItem, OutlineMember, SymbolType};
use crate::options::OutlineExtractorOptions;

const POTENTIAL_KINDS_INVARIANT: &str =
  "compiled outline rules must have potential kinds because RuleConfig rejects unconstrained rules";

/// Runtime outline extractors organized for a shared item traversal.
pub struct CombinedExtractors<L: Language> {
  /// Top-level item extractors matched during the file-wide AST traversal.
  item_extractors: Vec<ItemExtractor<L>>,
  /// Dense node-kind index into `item_extractors`; shared across the whole file.
  item_kind_index: Vec<Vec<usize>>,
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

impl<L: Language> Clone for ScopedMemberExtractors<'_, L> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<L: Language> Copy for ScopedMemberExtractors<'_, L> {}

#[derive(Default)]
struct MemberExtractorIndex {
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
    let member_index_by_parent = member_index_by_parent(&member_extractors);
    Self {
      item_extractors,
      item_kind_index,
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

  fn item_extractors_for_kind(&self, kind: u16) -> impl Iterator<Item = &ItemExtractor<L>> {
    self
      .item_kind_index
      .get(kind as usize)
      .map(Vec::as_slice)
      .unwrap_or(&[])
      .iter()
      .map(|&idx| &self.item_extractors[idx])
  }

  pub fn extract<'a, 'tree>(
    &'a self,
    root: Node<'tree, StrDoc<L>>,
  ) -> impl Iterator<Item = OutlineItem<'tree>> + use<'a, 'tree, L>
  where
    L: LanguageExt,
  {
    OutlineItemIter {
      combined: self,
      traversal: Prune::new(&root),
    }
  }

  fn match_item<'tree>(
    &self,
    node: &Node<'tree, StrDoc<L>>,
  ) -> Option<(&ItemExtractor<L>, NodeMatch<'tree, StrDoc<L>>)>
  where
    L: LanguageExt,
  {
    for extractor in self.item_extractors_for_kind(node.kind_id()) {
      if let Some(matched) = extractor.match_node(node) {
        return Some((extractor, matched));
      }
    }
    None
  }
}

impl<'a, L: Language> ScopedMemberExtractors<'a, L> {
  fn extract_member_with_rule<'tree>(
    &self,
    node: &Node<'tree, StrDoc<L>>,
  ) -> Option<(&'a MemberExtractor<L>, OutlineMember<'tree>)>
  where
    L: LanguageExt,
  {
    let kinds = self.index.kind_mapping.get(&node.kind_id())?;
    for &idx in kinds {
      let extractor = &self.extractors[idx];
      if let Some(matched) = extractor.match_node(node) {
        let member = extractor.extract(&matched, Vec::new());
        return Some((extractor, member));
      }
    }
    None
  }
}

struct OutlineItemIter<'a, 'tree, L: LanguageExt> {
  combined: &'a CombinedExtractors<L>,
  traversal: Prune<'tree, L>,
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
    let combined = self.combined;
    let item_subtree = self.traversal.current_subtree();
    let Some((extractor, node_match)) = combined.match_item(&node) else {
      self.traversal.descend();
      return None;
    };
    let members = self.collect_members_for_item(&extractor.common.rule.id, item_subtree);
    let item = extractor.extract(&node_match, members);
    combined.options.keep_item(&item).then_some(item)
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
      self.combined,
      &mut self.traversal,
      member_extractors,
      &self.combined.options,
      item_subtree,
    )
  }
}

fn validate_parent_rule_ids<L>(
  extractors: &[SerializableOutlineRule<L>],
) -> Result<(), OutlineRuleError> {
  let mut rule_ids = HashSet::new();
  for extractor in extractors {
    rule_ids.insert(extractor.common().id.as_str());
  }
  for extractor in extractors {
    let SerializableOutlineRule::Member(member) = extractor else {
      continue;
    };
    for parent_id in &member.parent_rule_ids {
      // A parent may be an item rule or another member rule: a member that is
      // itself a declaration may scope its own members (the override path).
      if !rule_ids.contains(parent_id.as_str()) {
        return Err(OutlineRuleError::UnknownParentRuleId {
          rule_id: member.common.id.clone(),
          parent_id: parent_id.clone(),
        });
      }
    }
  }
  Ok(())
}

/// Whether a symbol kind declares structure of its own: the member is itself a
/// declaration (a nested type, class, interface, enum, struct or object), not a
/// leaf like a field or a method.
fn has_members(symbol_type: SymbolType) -> bool {
  matches!(
    symbol_type,
    SymbolType::Class
      | SymbolType::Interface
      | SymbolType::Enum
      | SymbolType::Struct
      | SymbolType::Object
  )
}

fn collect_scoped_members<'a, 'tree, L: LanguageExt>(
  combined: &'a CombinedExtractors<L>,
  traversal: &mut Prune<'tree, L>,
  member_extractors: ScopedMemberExtractors<'a, L>,
  options: &OutlineExtractorOptions,
  subtree: PruneSubtree<'tree>,
) -> Vec<OutlineMember<'tree>> {
  let mut members = vec![];
  while let Some(node) = traversal.current_node() {
    if traversal.has_left_subtree(subtree) {
      break;
    }
    let Some((extractor, member)) = member_extractors.extract_member_with_rule(&node) else {
      traversal.descend();
      continue;
    };
    if !options.keep_member(&member) {
      traversal.skip_subtree();
      continue;
    }
    // A member that is itself a declaration keeps its own structure: descend
    // with the scope declared for its rule, falling back to the enclosing scope
    // so a nested declaration is read with the same rules as its container.
    let children = if has_members(extractor.common.symbol_type) {
      let scope = combined
        .member_scope_for(&extractor.common.rule.id)
        .unwrap_or(member_extractors);
      let child_subtree = traversal.current_subtree();
      traversal.descend();
      collect_scoped_members(combined, traversal, scope, options, child_subtree)
    } else {
      traversal.skip_subtree();
      vec![]
    };
    members.push(OutlineMember {
      members: children,
      ..member
    });
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
      .member_scope_for("ts-function")
      .expect("member extractors should exist");
    let identifier = SupportLang::TypeScript.ast_grep("let member = 1;");
    let identifier = identifier
      .root()
      .dfs()
      .find(|node| node.kind() == "identifier")
      .expect("fixture has an identifier");
    let identifier_member = member_extractors
      .extract_member_with_rule(&identifier)
      .map(|(extractor, _)| extractor);

    assert!(combined.member_scope_for("missing").is_none());
    assert_eq!(item_extractors.len(), 1);
    assert_eq!(item_extractors[0].common.rule.id, "ts-function");
    let identifier_member = identifier_member.expect("identifier matches ts-member");
    assert_eq!(identifier_member.common.rule.id, "ts-member");
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
  fn nests_members_of_a_member_that_is_itself_a_declaration() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: java-class
language: Java
role: item
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-nested-class
language: Java
role: member
parentRuleIds: [java-class]
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-member-method
language: Java
role: member
parentRuleIds: [java-class, java-nested-class]
symbolType: method
rule:
  all:
    - kind: method_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
"#,
    )
    .expect("a member rule may be a parent of another member rule");

    let combined = CombinedExtractors::try_from(extractors, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::Java.ast_grep(
      r#"
class Outer {
  class Inner {
    void innerMethod() {}
  }
  void outerMethod() {}
}
"#,
    );
    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    assert_eq!(items.len(), 1, "{items:?}");
    let outer = &items[0];
    let names = outer
      .members
      .iter()
      .map(|m| m.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(names, vec!["Inner", "outerMethod"], "{outer:?}");
    let inner = &outer.members[0];
    let nested = inner
      .members
      .iter()
      .map(|m| m.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(nested, vec!["innerMethod"], "{inner:?}");
  }

  #[test]
  fn prunes_a_member_that_is_not_a_declaration() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: java-class
language: Java
role: item
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-member-method
language: Java
role: member
parentRuleIds: [java-class]
symbolType: method
rule:
  all:
    - kind: method_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
"#,
    )
    .expect("rules compile");

    let combined = CombinedExtractors::try_from(extractors, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::Java.ast_grep(
      r#"
class Outer {
  void outerMethod() {
    class Local {}
  }
}
"#,
    );
    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    let outer = &items[0];
    let names = outer
      .members
      .iter()
      .map(|m| m.entry.name.as_ref())
      .collect::<Vec<_>>();
    // A callable is not a declaration container: its subtree is still pruned,
    // so a declaration inside a body stays invisible (unchanged behaviour).
    assert_eq!(names, vec!["outerMethod"], "{outer:?}");
    assert!(outer.members[0].members.is_empty(), "{outer:?}");
  }

  #[test]
  fn a_filtered_out_container_takes_its_members_with_it() {
    let extractors = parse_outline_rules::<SupportLang>(
      r#"
id: java-class
language: Java
role: item
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-nested-class
language: Java
role: member
parentRuleIds: [java-class]
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
isPublic:
  has:
    kind: modifiers
    has:
      pattern: public
---
id: java-member-method
language: Java
role: member
parentRuleIds: [java-class]
symbolType: method
rule:
  all:
    - kind: method_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
"#,
    )
    .expect("rules compile");
    let options = OutlineExtractorOptions {
      members: Some(crate::options::OutlineMemberOptions {
        public: crate::options::OutlineFlagFilter::Yes,
        ..Default::default()
      }),
      ..Default::default()
    };
    let combined = CombinedExtractors::try_from_rules(extractors, options, &Default::default())
      .expect("extractors should parse");
    let grep = SupportLang::Java.ast_grep(
      r#"
class Outer {
  public class PublicInner {
    void keptMethod() {}
  }
  class PrivateInner {
    void hiddenMethod() {}
  }
}
"#,
    );
    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    let outer = &items[0];
    let names = outer
      .members
      .iter()
      .map(|m| m.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(names, vec!["PublicInner"], "{outer:?}");
    let inner = outer.members[0]
      .members
      .iter()
      .map(|c| c.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(inner, vec!["keptMethod"], "{outer:?}");
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
