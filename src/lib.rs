use std::ops::{Deref, DerefMut};
use ts_parser::{Edit, perform_edit};

mod ts_parser;
mod language;
mod matcher;
mod meta_var;
mod node;
mod pattern;
mod replacer;
pub mod rule;

pub use pattern::Pattern;
pub use node::Node;
pub use meta_var::MetaVarMatcher;

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
    /*
    use super::*;
    #[test]
    fn test_replace() {
    let mut node = Semgrep::new("var a = 1;");
    node.replace("var $_$ = $_$", "let $_$ = $_$");
    let replaced = Semgrep::generate(&node);
    assert_eq!(replaced, "let a = 1");
    }
    */
}
