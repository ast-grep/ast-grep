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
pub mod traversal;

#[doc(hidden)]
pub mod pinned;

mod match_tree;
mod node;

pub use language::Language;
pub use matcher::{Matcher, NodeMatch, Pattern, PatternError};
pub use node::Node;
pub use source::{Doc, StrDoc};

use replacer::Replacer;

use node::Root;
use source::{Edit, TSParseError};

#[derive(Clone)]
pub struct AstGrep<D: Doc> {
  #[doc(hidden)]
  pub inner: Root<D>,
}
impl<D: Doc> AstGrep<D> {
  pub fn root(&self) -> Node<D> {
    self.inner.root()
  }

  pub fn edit(&mut self, edit: Edit<D::Source>) -> Result<&mut Self, TSParseError> {
    self.inner.do_edit(edit)?;
    Ok(self)
  }

  pub fn replace<M: Matcher<D::Lang>, R: Replacer<D>>(
    &mut self,
    pattern: M,
    replacer: R,
  ) -> Result<bool, TSParseError> {
    if let Some(edit) = self.root().replace(pattern, replacer) {
      self.edit(edit)?;
      Ok(true)
    } else {
      Ok(false)
    }
  }

  pub fn lang(&self) -> &D::Lang {
    self.inner.lang()
  }

  /// Use this method to avoid expensive string encoding overhead
  /// TODO: add more documents on what is happening
  pub fn doc(d: D) -> Self {
    Self {
      inner: Root::doc(d),
    }
  }
}

impl<L: Language> AstGrep<StrDoc<L>> {
  pub fn new<S: AsRef<str>>(src: S, lang: L) -> Self {
    Self {
      inner: Root::new(src.as_ref(), lang),
    }
  }

  /*
  pub fn customized<C: Content>(content: C, lang: L) -> Result<Self, TSParseError> {
    Ok(Self {
      inner: Root::customized(content, lang)?,
    })
  }
  */
  pub fn source(&self) -> &str {
    self.inner.doc.get_source().as_str()
  }

  pub fn generate(self) -> String {
    self.inner.doc.src
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use language::Tsx;
  use ops::Op;

  pub type Result = std::result::Result<(), TSParseError>;

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
