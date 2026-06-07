use std::ops::Range;

use ast_grep_core::Position;

/// Outline symbol category.
///
/// The names follow LSP `DocumentSymbol.kind`, but ast-grep stores the symbolic
/// category directly instead of exposing LSP numeric values.
/// See https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/#textDocument_documentSymbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// A file-level relationship between an outline item and its source file.
///
/// Roles are facets, not mutually exclusive categories.
/// One item can be both a definition and an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineRole {
  /// A symbol declared or defined in the file.
  Definition,
  /// A symbol or module brought into the file from elsewhere.
  Import,
  /// A symbol or module exposed as part of the file's outward surface.
  Export,
}

/// Compact role flags for one outline item.
///
/// A single item can have multiple roles.
/// e.g. `pub struct Foo` is both a definition and an export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutlineRoles(u8);

impl OutlineRoles {
  const DEFINITION: u8 = 1 << 0;
  const IMPORT: u8 = 1 << 1;
  const EXPORT: u8 = 1 << 2;

  /// Creates role flags from zero or more roles.
  pub fn new(roles: impl IntoIterator<Item = OutlineRole>) -> Self {
    roles
      .into_iter()
      .fold(Self::default(), |roles, role| roles.with(role))
  }

  /// Returns true when the role flag is present.
  pub fn contains(self, role: OutlineRole) -> bool {
    self.0 & Self::flag(role) != 0
  }

  /// Returns true when all role flags in `roles` are present.
  pub fn contains_all(self, roles: OutlineRoles) -> bool {
    self.0 & roles.0 == roles.0
  }

  /// Returns true when no role flag is present.
  pub fn is_empty(self) -> bool {
    self.0 == 0
  }

  fn with(mut self, role: OutlineRole) -> Self {
    self.0 |= Self::flag(role);
    self
  }

  fn flag(role: OutlineRole) -> u8 {
    match role {
      OutlineRole::Definition => Self::DEFINITION,
      OutlineRole::Import => Self::IMPORT,
      OutlineRole::Export => Self::EXPORT,
    }
  }
}

impl<const N: usize> From<[OutlineRole; N]> for OutlineRoles {
  fn from(roles: [OutlineRole; N]) -> Self {
    Self::new(roles)
  }
}

/// One extracted outline item.
///
/// The item borrows textual fields from source text. Rendering can recover the
/// display line from `range`, so the model does not store a separate signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineItem<'a> {
  pub name: &'a str,
  pub symbol_type: SymbolType,
  pub roles: OutlineRoles,
  pub range: Range<Position>,
}

impl<'a> OutlineItem<'a> {
  /// Creates an outline item from borrowed source text and source range.
  pub fn new(
    name: &'a str,
    symbol_type: SymbolType,
    roles: impl Into<OutlineRoles>,
    range: Range<Position>,
  ) -> Self {
    Self {
      name,
      symbol_type,
      roles: roles.into(),
      range,
    }
  }

  /// Returns true when the item has the requested role.
  pub fn has_role(&self, role: OutlineRole) -> bool {
    self.roles.contains(role)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_range() -> Range<Position> {
    Position::new(0, 0, 0)..Position::new(0, 10, 10)
  }

  #[test]
  fn outline_item_can_have_multiple_roles() {
    let item = OutlineItem::new(
      "Foo",
      SymbolType::Struct,
      [OutlineRole::Definition, OutlineRole::Export],
      test_range(),
    );

    assert!(item.has_role(OutlineRole::Definition));
    assert!(item.has_role(OutlineRole::Export));
    assert!(!item.has_role(OutlineRole::Import));
  }

  #[test]
  fn outline_roles_are_unique_flags() {
    let roles = OutlineRoles::from([
      OutlineRole::Definition,
      OutlineRole::Export,
      OutlineRole::Definition,
    ]);

    assert!(roles.contains(OutlineRole::Definition));
    assert!(roles.contains(OutlineRole::Export));
    assert!(!roles.contains(OutlineRole::Import));
    assert_eq!(roles.0, OutlineRoles::DEFINITION | OutlineRoles::EXPORT);
  }

  #[test]
  fn outline_roles_can_match_all_requested_roles() {
    let roles = OutlineRoles::from([OutlineRole::Definition, OutlineRole::Export]);
    let definition_export = OutlineRoles::from([OutlineRole::Definition, OutlineRole::Export]);
    let import_export = OutlineRoles::from([OutlineRole::Import, OutlineRole::Export]);

    assert!(roles.contains_all(definition_export));
    assert!(!roles.contains_all(import_export));
  }

  #[test]
  fn empty_outline_roles_have_no_flags() {
    let roles = OutlineRoles::default();

    assert!(roles.is_empty());
    assert!(!roles.contains(OutlineRole::Definition));
  }
}
