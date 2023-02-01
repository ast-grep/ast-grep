pub mod language;
pub mod matcher;
pub mod meta_var;
pub mod ops;
pub mod source;
pub mod traversal;

#[doc(hidden)]
pub mod pinned;

mod match_tree;
mod node;
mod replacer;
mod ts_parser;

pub use language::Language;
pub use matcher::{Matcher, NodeMatch, Pattern, PatternError};
pub use node::Node;
pub use replacer::replace_meta_var_in_string;

use crate::replacer::Replacer;
use node::Root;
use source::Content;
use ts_parser::{Edit, TSParseError};

#[derive(Clone)]
pub struct AstGrep<L: Language> {
  inner: Root<L>,
}

impl<L: Language> AstGrep<L> {
  pub fn new<S: AsRef<str>>(src: S, lang: L) -> Self {
    Self {
      inner: Root::new(src.as_ref(), lang),
    }
  }

  pub fn customized<C: Content>(content: C, lang: L) -> Result<Self, TSParseError> {
    Ok(Self {
      inner: Root::customized(content, lang)?,
    })
  }

  pub fn source(&self) -> &str {
    &self.inner.source
  }

  pub fn root(&self) -> Node<L> {
    self.inner.root()
  }

  pub fn edit(&mut self, edit: Edit) -> Result<&mut Self, TSParseError> {
    self.inner.do_edit(edit)?;
    Ok(self)
  }

  pub fn replace<M: Matcher<L>, R: Replacer<L>>(
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

  pub fn lang(&self) -> &L {
    &self.inner.lang
  }

  pub fn generate(self) -> String {
    self.inner.source.to_string()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use language::Tsx;
  use ops::Op;

  #[test]
  fn test_replace() {
    let mut ast_grep = Tsx.ast_grep("var a = 1; let b = 2;");
    ast_grep.replace("var $A = $B", "let $A = $B").unwrap();
    let source = ast_grep.generate();
    assert_eq!(source, "let a = 1; let b = 2;"); // note the semicolon
  }

  #[test]
  fn test_replace_by_rule() {
    let rule = Op::either("let a = 123").or("let b = 456");
    let mut ast_grep = Tsx.ast_grep("let a = 123");
    let replaced = ast_grep.replace(rule, "console.log('it works!')").unwrap();
    assert!(replaced);
    let source = ast_grep.generate();
    assert_eq!(source, "console.log('it works!')");
  }

  #[test]
  fn test_replace_unnamed_node() {
    // ++ and -- is unnamed node in tree-sitter javascript
    let mut ast_grep = Tsx.ast_grep("c++");
    ast_grep.replace("$A++", "$A--").unwrap();
    let source = ast_grep.generate();
    assert_eq!(source, "c--");
  }

  #[test]
  fn test_replace_trivia() {
    let mut ast_grep = Tsx.ast_grep("var a = 1 /*haha*/;");
    ast_grep.replace("var $A = $B", "let $A = $B").unwrap();
    let source = ast_grep.generate();
    assert_eq!(source, "let a = 1 /*haha*/;"); // semicolon

    let mut ast_grep = Tsx.ast_grep("var a = 1; /*haha*/");
    ast_grep.replace("var $A = $B", "let $A = $B").unwrap();
    let source = ast_grep.generate();
    assert_eq!(source, "let a = 1; /*haha*/");
  }
}
