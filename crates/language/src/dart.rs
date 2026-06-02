use ast_grep_core::Language;
use ast_grep_core::matcher::{KindMatcher, Pattern, PatternBuilder, PatternError, PatternNode};
use ast_grep_core::node::Root;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};

/// Dart's tree-sitter grammar only accepts declarations at the top level.
/// Expression-level patterns (function calls, member access, assignments, etc.)
/// fail to parse when given as bare source, since they are not valid top-level
/// Dart constructs. This implementation wraps such patterns inside a function
/// body and extracts the inner node so that expression/statement patterns work.
#[derive(Clone, Copy, Debug)]
pub struct Dart;

const WRAPPER_PREFIX: &str = "void _() {\n";
const WRAPPER_SUFFIX: &str = "\n}";

impl Language for Dart {
  fn kind_to_id(&self, kind: &str) -> u16 {
    self.get_ts_language().id_for_node_kind(kind, true)
  }
  fn field_to_id(&self, field: &str) -> Option<u16> {
    self
      .get_ts_language()
      .field_id_for_name(field)
      .map(|f| f.get())
  }
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    let src = builder.src();
    // Check if the pattern parses cleanly as a top-level construct.
    // Dart's grammar can misinterpret expressions as declarations
    // (e.g. `print($A)` becomes a function_signature, `AppIcons.$FIELD`
    // becomes a type reference).  If the direct parse has any ERROR or
    // MISSING nodes, try wrapping in a function body instead.
    if self.should_try_wrapping(src) {
      if let Some(pattern) = self.try_wrapped_pattern(src) {
        return Ok(pattern);
      }
    }
    builder.build(|s| StrDoc::try_new(s, *self))
  }
}
/// Dart keywords that begin statements but are not valid function/type names.
/// When tree-sitter misinterprets `if (...)` as a function declaration named
/// "if", this list lets us detect and correct the misparse.
const DART_STATEMENT_KEYWORDS: &[&str] = &[
  "if", "else", "for", "while", "do", "switch", "try", "throw", "rethrow", "return", "break",
  "continue", "yield", "await", "assert",
];

impl Dart {
  /// Decides whether the direct (top-level) parse is likely wrong and
  /// wrapping should be attempted.  Four independent signals:
  ///  1. Root node itself is ERROR (total misparse)
  ///  2. A direct child of source_file is ERROR
  ///  3. Any MISSING node in the tree (grammar hallucinated a token like `;`)
  ///  4. Source starts with a statement keyword misread as a function name
  fn should_try_wrapping(&self, src: &str) -> bool {
    let Ok(doc) = StrDoc::try_new(src, *self) else {
      return true;
    };
    let root = Root::doc(doc);
    let root_node = root.root();
    if root_node.is_error() {
      return true;
    }
    if root_node.children().any(|c| c.is_error()) {
      return true;
    }
    if Self::subtree_has_missing(&root_node) {
      return true;
    }
    let first_word = src
      .split(|c: char| !c.is_alphanumeric() && c != '_')
      .next()
      .unwrap_or("");
    DART_STATEMENT_KEYWORDS.contains(&first_word)
  }

  /// Returns true if any node in the subtree is a MISSING (phantom) node
  /// inserted by tree-sitter to recover from a parse error.
  fn subtree_has_missing<D: ast_grep_core::Doc>(node: &ast_grep_core::Node<'_, D>) -> bool {
    if node.is_missing() {
      return true;
    }
    node.children().any(|c| Self::subtree_has_missing(&c))
  }

  /// Wraps `src` inside a function body and tries to extract the inner
  /// statement or expression as a pattern. Tries without a trailing semicolon
  /// first (for compound statements like if/for/while), then with one (for
  /// expression statements like function calls and assignments).
  fn try_wrapped_pattern(&self, src: &str) -> Option<Pattern> {
    // Try without semicolon first (statement patterns like if/for/while),
    // then with semicolon (expression patterns that need `;` to form a
    // complete statement in Dart).
    let candidates = [
      format!("{WRAPPER_PREFIX}{src}{WRAPPER_SUFFIX}"),
      format!("{WRAPPER_PREFIX}{src};{WRAPPER_SUFFIX}"),
    ];
    for wrapped in &candidates {
      if let Some(pattern) = self.try_extract_inner(wrapped) {
        return Some(pattern);
      }
    }
    None
  }

  /// Parses the wrapped source, locates the wrapper function's block, and
  /// extracts the first statement/expression as a [`Pattern`]. Returns `None`
  /// if the parse contains errors or no usable inner node is found.
  fn try_extract_inner(&self, wrapped: &str) -> Option<Pattern> {
    let doc = StrDoc::try_new(wrapped, *self).ok()?;
    let root = Root::doc(doc);
    let root_node = root.root();
    let block_kind = KindMatcher::new("block", *self);
    let block = root_node.find(&block_kind)?;
    if block.children().any(|c| c.is_error()) {
      return None;
    }
    let inner = block.children().find(|c| c.is_named())?;
    // Unwrap expression_statement to get the inner expression so that
    // `print($A)` matches `call_expression` nodes directly.
    let target = if inner.kind() == "expression_statement" {
      inner.children().find(|c| c.is_named())?
    } else {
      inner
    };
    let mut pattern = Pattern::from(target);
    if pattern.has_error() {
      return None;
    }
    // Dart's grammar sometimes parses `{ $$$BODY }` as set_or_map_literal
    // instead of block (ambiguity when content looks like an expression).
    // Fix up the pattern tree so it matches real block nodes.
    let set_or_map_id = self.kind_to_id("set_or_map_literal");
    let block_id = self.kind_to_id("block");
    let expr_stmt_id = self.kind_to_id("expression_statement");
    Self::fix_set_literal_to_block(&mut pattern.node, set_or_map_id, block_id, expr_stmt_id);
    Some(pattern)
  }

  /// Recursively replace set_or_map_literal nodes with block nodes in the
  /// pattern tree. This fixes the Dart grammar ambiguity where `{ $$$BODY }`
  /// is parsed as a set literal instead of a block in statement contexts.
  fn fix_set_literal_to_block(
    node: &mut PatternNode,
    set_or_map_id: u16,
    block_id: u16,
    expr_stmt_id: u16,
  ) {
    if let PatternNode::Internal { kind_id, children } = node {
      // An expression_statement wrapping a set_or_map_literal should
      // become just a block (remove the expression_statement wrapper).
      if *kind_id == expr_stmt_id {
        if let Some(first_named) = children.iter().position(
          |c| matches!(c, PatternNode::Internal { kind_id: k, .. } if *k == set_or_map_id),
        ) {
          if let PatternNode::Internal {
            kind_id: inner_kind,
            children: inner_children,
          } = &mut children[first_named]
          {
            *inner_kind = block_id;
            // Promote the block to replace the expression_statement
            let block_children = std::mem::take(inner_children);
            *kind_id = block_id;
            *children = block_children;
            return;
          }
        }
      }
      if *kind_id == set_or_map_id {
        *kind_id = block_id;
      }
      for child in children.iter_mut() {
        Self::fix_set_literal_to_block(child, set_or_map_id, block_id, expr_stmt_id);
      }
    }
  }
}

impl LanguageExt for Dart {
  fn get_ts_language(&self) -> TSLanguage {
    crate::parsers::language_dart()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::{test_match_lang, test_non_match_lang, test_replace_lang};

  fn test_match(query: &str, source: &str) {
    test_match_lang(query, source, Dart);
  }

  fn test_non_match(query: &str, source: &str) {
    test_non_match_lang(query, source, Dart);
  }

  #[test]
  fn test_dart_class() {
    test_match("class $A {}", "class Foo {}");
    test_non_match("class $A {}", "class Foo { int x = 1; }");
  }

  #[test]
  fn test_dart_class_with_body() {
    test_match("class $A { $$$BODY }", "class Foo { int x = 1; }");
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
    test_replace_lang(src, pattern, replacer, Dart)
  }

  #[test]
  fn test_dart_replace() {
    let ret = test_replace("class Foo {}", "class $A {}", "class $A extends Base {}");
    assert_eq!(ret, "class Foo extends Base {}");
  }

  // -- Expression-level patterns (new tests) --

  #[test]
  fn test_dart_function_call() {
    test_match("print($A)", "void main() { print(123); }");
    test_match("print($$$)", "void main() { print(1, 2, 3); }");
    test_non_match("print($A)", "void main() { debugPrint(123); }");
  }

  #[test]
  fn test_dart_method_call() {
    test_match("$X.add($A)", "void f() { list.add(1); }");
    test_non_match("$X.add($A)", "void f() { list.remove(1); }");
  }

  #[test]
  fn test_dart_named_constructor() {
    test_match(
      "OverlayButton.bubble($$$)",
      "class Foo { Widget build() { return OverlayButton.bubble(onTap: f); } }",
    );
    test_non_match(
      "OverlayButton.bubble($$$)",
      "class Foo { Widget build() { return OverlayButton.story(onTap: f); } }",
    );
  }

  #[test]
  fn test_dart_assignment() {
    test_match("$A = $B", "void f() { x = 42; }");
  }

  #[test]
  fn test_dart_variable_declaration() {
    test_match("var $A = $B", "void f() { var x = 42; }");
  }

  #[test]
  fn test_dart_import() {
    // Dart imports use string literals, so $URI must be quoted.
    // Bare `import $URI` is misinterpreted by tree-sitter as a
    // variable declaration (type: import, name: $URI).
    test_match("import '$URI'", "import 'package:flutter/material.dart';");
  }

  #[test]
  fn test_dart_top_level_function() {
    test_match(
      "void $NAME($$$) { $$$BODY }",
      "void main() { print('hello'); }",
    );
  }

  #[test]
  fn test_dart_member_access() {
    test_match("AppIcons.$FIELD", "void f() { var x = AppIcons.heart; }");
  }

  #[test]
  fn test_dart_return_statement() {
    test_match("return $A", "int f() { return 42; }");
  }

  #[test]
  fn test_dart_if_statement() {
    test_match(
      "if ($COND) { $$$BODY }",
      "void f() { if (x > 0) { print(x); } }",
    );
  }

  #[test]
  fn test_dart_expression_replace() {
    let ret = test_replace("void f() { print(123); }", "print($A)", "debugPrint($A)");
    assert_eq!(ret, "void f() { debugPrint(123); }");
  }
}
