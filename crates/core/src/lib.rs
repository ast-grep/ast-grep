/*!
This module contains the core library for ast-grep.

It provides APIs for parsing, traversing, searching and replacing tree-sitter nodes.
Usually you will only need ast-grep CLI instead of this crate.
But if you want to use ast-grep as a library, this is the right place.
*/

pub mod language;
pub mod matcher;
pub mod meta_var;
pub mod ops;
pub mod replacer;
pub mod source;
#[cfg(feature = "tree-sitter")]
pub mod tree_sitter;

#[doc(hidden)]
pub mod pinned;

mod match_tree;
mod node;

pub use language::Language;
pub use match_tree::MatchStrictness;
pub use matcher::{Matcher, NodeMatch, Pattern, PatternError};
pub use node::{Node, Position};
pub use source::Doc;

use node::Root;

pub type AstGrep<D> = Root<D>;

#[cfg(test)]
mod test {
  use super::*;
  use crate::tree_sitter::LanguageExt;
  use language::Tsx;
  use ops::Op;

  pub type Result = std::result::Result<(), String>;

  #[test]
  fn test_replace() -> Result {
    let mut ast_grep = Tsx.ast_grep("var a = 1; let b = 2;");
    ast_grep.replace("var $A = $B", "let $A = $B")?;
    let source = ast_grep.generate();
    assert_eq!(source, "let a = 1; let b = 2;"); // note the semicolon
    Ok(())
  }

  #[test]
  fn test_replace_by_rule() -> Result {
    let rule = Op::either("let a = 123").or("let b = 456");
    let mut ast_grep = Tsx.ast_grep("let a = 123");
    let replaced = ast_grep.replace(rule, "console.log('it works!')")?;
    assert!(replaced);
    let source = ast_grep.generate();
    assert_eq!(source, "console.log('it works!')");
    Ok(())
  }

  #[test]
  fn test_replace_unnamed_node() -> Result {
    // ++ and -- is unnamed node in tree-sitter javascript
    let mut ast_grep = Tsx.ast_grep("c++");
    ast_grep.replace("$A++", "$A--")?;
    let source = ast_grep.generate();
    assert_eq!(source, "c--");
    Ok(())
  }

  #[test]
  fn test_replace_trivia() -> Result {
    let mut ast_grep = Tsx.ast_grep("var a = 1 /*haha*/;");
    ast_grep.replace("var $A = $B", "let $A = $B")?;
    let source = ast_grep.generate();
    assert_eq!(source, "let a = 1 /*haha*/;"); // semicolon

    let mut ast_grep = Tsx.ast_grep("var a = 1; /*haha*/");
    ast_grep.replace("var $A = $B", "let $A = $B")?;
    let source = ast_grep.generate();
    assert_eq!(source, "let a = 1; /*haha*/");
    Ok(())
  }

  #[test]
  fn test_replace_trivia_with_skipped() -> Result {
    let mut ast_grep = Tsx.ast_grep("return foo(1, 2,) /*haha*/;");
    ast_grep.replace("return foo($A, $B)", "return bar($A, $B)")?;
    let source = ast_grep.generate();
    assert_eq!(source, "return bar(1, 2) /*haha*/;"); // semicolon
    Ok(())
  }
}
