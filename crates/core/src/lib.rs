use std::ops::{Deref, DerefMut};

mod language;
mod matcher;
mod meta_var;
mod node;
mod pattern;
mod replacer;
mod rule;
mod ts_parser;

pub use meta_var::MetaVarMatcher;
pub use node::Node;
pub use pattern::Pattern;
pub use rule::Rule;

use crate::{replacer::Replacer, rule::PositiveMatcher};
use ts_parser::{perform_edit, Edit};

pub struct AstGrep {
    root: Root,
}

pub struct Root {
    inner: ts_parser::Tree,
    source: String,
}

impl Root {
    fn new(src: &str) -> Self {
        Self {
            inner: ts_parser::parse(src, None),
            source: src.into(),
        }
    }

    pub fn root(&self) -> Node {
        Node {
            inner: self.inner.root_node(),
            source: &self.source,
        }
    }

    pub fn edit(&mut self, edit: Edit) -> &mut Self {
        let input = unsafe { self.source.as_mut_vec() };
        let input_edit = perform_edit(&mut self.inner, input, &edit);
        self.inner.edit(&input_edit);
        self.inner = ts_parser::parse(&self.source, Some(&self.inner));
        self
    }


    pub fn replace<M: PositiveMatcher, R: Replacer>(&mut self, pattern: M, replacer: R) -> bool {
        if let Some(edit) = self.root().replace(pattern, replacer) {
            self.edit(edit);
            true
        } else {
            false
        }
    }
}

// creational API
impl AstGrep {
    pub fn new<S: AsRef<str>>(source: S) -> Self {
        Self {
            root: Root::new(source.as_ref()),
        }
    }

    pub fn generate(self) -> String {
        self.root.source
    }
}

impl Deref for AstGrep {
    type Target = Root;
    fn deref(&self) -> &Self::Target {
        &self.root
    }
}
impl DerefMut for AstGrep {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_replace() {
        let mut ast_grep = AstGrep::new("var a = 1; let b = 2;");
        ast_grep.replace("var $A = $B", "let $A = $B");
        let source = ast_grep.generate();
        assert_eq!(source, "let a = 1; let b = 2;"); // note the semicolon
    }

    #[test]
    fn test_replace_by_rule() {
        let rule = Rule::either("let a = 123").or("let b = 456").build();
        let mut ast_grep = AstGrep::new("let a = 123");
        let replaced = ast_grep.replace(rule, "console.log('it works!')");
        assert!(replaced);
        let source = ast_grep.generate();
        assert_eq!(source, "console.log('it works!')");
    }

    #[test]
    fn test_replace_trivia() {
        let mut ast_grep = AstGrep::new("var a = 1 /*haha*/;");
        ast_grep.replace("var $A = $B", "let $A = $B");
        let source = ast_grep.generate();
        assert_eq!(source, "let a = 1;"); // semicolon

        let mut ast_grep = AstGrep::new("var a = 1; /*haha*/");
        ast_grep.replace("var $A = $B", "let $A = $B");
        let source = ast_grep.generate();
        assert_eq!(source, "let a = 1; /*haha*/");
    }
}
