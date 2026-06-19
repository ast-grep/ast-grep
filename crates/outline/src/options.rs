//! Extraction options for outline rule compilation and extraction.
//!
//! These options are evaluated in two phases: first to prune serializable rules
//! before compilation, then to filter concrete entries after extraction.

use crate::extractor::{SerializableOutlineRule, SerializablePredicate};
use crate::model::{OutlineItem, OutlineMember, SymbolType};

/// Options for compiling and extracting an outline.
#[derive(Clone, Debug)]
pub struct OutlineExtractorOptions {
  /// Top-level item symbol types to include. `None` accepts every symbol type.
  pub symbol_types: Option<Vec<SymbolType>>,
  /// Filter for whether a top-level item is an import.
  pub imports: OutlineFlagFilter,
  /// Filter for whether a top-level item is exported.
  pub exported: OutlineFlagFilter,
  /// Text detail to compute for top-level items.
  pub detail: OutlineEntryDetail,
  /// Member extraction options. `None` disables member extraction entirely.
  pub members: Option<OutlineMemberOptions>,
}

impl Default for OutlineExtractorOptions {
  fn default() -> Self {
    Self {
      symbol_types: None,
      imports: OutlineFlagFilter::Any,
      exported: OutlineFlagFilter::Any,
      detail: OutlineEntryDetail::Signature,
      members: Some(OutlineMemberOptions::default()),
    }
  }
}

/// Options that apply to direct item members.
#[derive(Clone, Debug)]
pub struct OutlineMemberOptions {
  /// Filter for whether a member is public.
  pub public: OutlineFlagFilter,
  /// Text detail to compute for members.
  pub detail: OutlineEntryDetail,
}

impl Default for OutlineMemberOptions {
  fn default() -> Self {
    Self {
      public: OutlineFlagFilter::Any,
      detail: OutlineEntryDetail::Signature,
    }
  }
}

/// Ternary filter for flags derived from literals or runtime predicates.
#[derive(Clone, Copy, Debug, Default)]
pub enum OutlineFlagFilter {
  #[default]
  Any,
  Yes,
  No,
}

/// How much text to compute for each returned entry.
#[derive(Clone, Copy, Debug)]
pub enum OutlineEntryDetail {
  /// Compute only the entry name. Returned signatures should be empty.
  Name,
  /// Compute source-like signatures, falling back to the matched source line.
  Signature,
}

impl OutlineExtractorOptions {
  /// Compile-time rule retention for the first filtering phase.
  ///
  /// Use this method before compiling serializable rules. It is conservative
  /// for predicate-backed flags because their exact value is only known after a
  /// node match is extracted.
  /// Returns whether a serializable rule is worth compiling.
  pub fn retain_rule<L>(&self, rule: &SerializableOutlineRule<L>) -> bool {
    match rule {
      SerializableOutlineRule::Item(item) => {
        matches_symbol_type(item.common.symbol_type, self.symbol_types.as_deref())
          && self
            .imports
            .matches_predicate(item.is_import.as_ref(), false)
          && self
            .exported
            .matches_predicate(item.is_exported.as_ref(), true)
      }
      SerializableOutlineRule::Member(member) => {
        let Some(member_options) = &self.members else {
          return false;
        };
        member_options
          .public
          .matches_predicate(member.is_public.as_ref(), true)
      }
    }
  }

  /// Extraction-time item filtering for the second filtering phase.
  ///
  /// Returns whether a concrete extracted item should be returned.
  pub fn keep_item(&self, item: &OutlineItem) -> bool {
    matches_symbol_type(item.entry.symbol_type, self.symbol_types.as_deref())
      && self.imports.matches_value(item.is_import)
      && self.exported.matches_value(item.is_exported)
  }

  /// Extraction-time member filtering for the second filtering phase.
  ///
  /// Returns whether a concrete extracted member should be returned.
  pub fn keep_member(&self, member: &OutlineMember) -> bool {
    self
      .members
      .as_ref()
      .is_some_and(|options| options.public.matches_value(member.is_public))
  }
}

impl OutlineFlagFilter {
  fn matches_value(self, value: bool) -> bool {
    match self {
      Self::Any => true,
      Self::Yes => value,
      Self::No => !value,
    }
  }

  fn matches_predicate(self, predicate: Option<&SerializablePredicate>, default: bool) -> bool {
    match predicate {
      Some(SerializablePredicate::Literal(value)) => self.matches_value(*value),
      Some(SerializablePredicate::Rule(_)) => true,
      None => self.matches_value(default),
    }
  }
}

fn matches_symbol_type(symbol_type: SymbolType, filters: Option<&[SymbolType]>) -> bool {
  filters.is_none_or(|filters| filters.contains(&symbol_type))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::extractor::parse_outline_rules;
  use crate::model::{EntryRole, OutlineEntry, SourcePosition, SourceRange};
  use ast_grep_language::SupportLang;
  use std::borrow::Cow;

  fn rules(src: &str) -> Vec<SerializableOutlineRule<SupportLang>> {
    parse_outline_rules(src).expect("outline rules should parse")
  }

  fn range() -> SourceRange {
    SourceRange {
      byte_offset: 0..0,
      start: SourcePosition { line: 0, column: 0 },
      end: SourcePosition { line: 0, column: 0 },
    }
  }

  fn entry(role: EntryRole, symbol_type: SymbolType) -> OutlineEntry<'static> {
    OutlineEntry {
      role,
      symbol_type,
      name: Cow::Borrowed("name"),
      range: range(),
      signature: Cow::Borrowed("signature"),
      ast_kind: Cow::Borrowed("node"),
    }
  }

  #[test]
  fn retain_rule_filters_serializable_rules_before_compilation() {
    let rules = rules(
      r#"
id: ts-import
language: TypeScript
role: item
symbolType: module
rule:
  kind: import_statement
name: import
isImport: true
---
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME() {}
name: $NAME
isImport: false
---
id: ts-private-member
language: TypeScript
role: member
parentRuleIds: [ts-function]
symbolType: method
rule:
  kind: method_definition
name: method
isPublic: false
"#,
    );
    let import_options = OutlineExtractorOptions {
      imports: OutlineFlagFilter::Yes,
      ..Default::default()
    };
    let public_member_options = OutlineExtractorOptions {
      members: Some(OutlineMemberOptions {
        public: OutlineFlagFilter::Yes,
        ..Default::default()
      }),
      ..Default::default()
    };
    let no_member_options = OutlineExtractorOptions {
      members: None,
      ..Default::default()
    };

    assert!(import_options.retain_rule(&rules[0]));
    assert!(!import_options.retain_rule(&rules[1]));
    assert!(!public_member_options.retain_rule(&rules[2]));
    assert!(!no_member_options.retain_rule(&rules[2]));
  }

  #[test]
  fn keep_item_and_member_filter_extracted_entries() {
    let exported_function = OutlineItem {
      entry: entry(EntryRole::Item, SymbolType::Function),
      is_import: false,
      is_exported: true,
      members: vec![],
    };
    let private_member = OutlineMember {
      entry: entry(EntryRole::Member, SymbolType::Method),
      is_public: false,
    };
    let exported_options = OutlineExtractorOptions {
      exported: OutlineFlagFilter::Yes,
      ..Default::default()
    };
    let import_options = OutlineExtractorOptions {
      imports: OutlineFlagFilter::Yes,
      ..Default::default()
    };
    let public_member_options = OutlineExtractorOptions {
      members: Some(OutlineMemberOptions {
        public: OutlineFlagFilter::Yes,
        ..Default::default()
      }),
      ..Default::default()
    };

    assert!(exported_options.keep_item(&exported_function));
    assert!(!import_options.keep_item(&exported_function));
    assert!(!public_member_options.keep_member(&private_member));
  }
}
