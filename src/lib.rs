use std::ops::{Deref, DerefMut};
use ts_parser::{perform_edit, Edit};

mod language;
mod matcher;
mod meta_var;
mod node;
mod pattern;
mod replacer;
pub mod rule;
mod ts_parser;

pub use meta_var::MetaVarMatcher;
pub use node::Node;
pub use pattern::Pattern;

pub struct Semgrep {
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

    pub fn replace(&mut self, pattern: &str, replacer: &str) -> bool {
        if let Some(edit) = self.root().replace(pattern, replacer) {
            self.edit(edit);
            true
        } else {
            false
        }
    }
}

// creational API
impl Semgrep {
    pub fn new<S: AsRef<str>>(source: S) -> Self {
        Self {
            root: Root::new(source.as_ref()),
        }
    }
    pub fn generate(_n: &Node) -> String {
        todo!()
    }
}

impl Deref for Semgrep {
    type Target = Root;
    fn deref(&self) -> &Self::Target {
        &self.root
    }
}
impl DerefMut for Semgrep {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_replace() {
        let mut semgrep = Semgrep::new("var a = 1;");
        semgrep.replace("var $A = $B", "let $A = $B");
        assert_eq!(semgrep.source, "let a = 1");
    }
}
