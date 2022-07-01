use std::ops::{Deref, DerefMut};
use std::rc::Rc;

mod ts_parser;
mod language;
mod matcher;
mod meta_var;
mod node;
mod pattern;
pub mod rule;

pub use pattern::Pattern;
pub use node::Node;
pub use meta_var::MetaVarMatcher;

pub struct Semgrep {
    root: Root,
}

pub struct Root {
    inner: ts_parser::Tree,
    source: Rc<String>,
}

impl Root {
    fn new(src: &str) -> Self {
        Self {
            inner: ts_parser::parse(src),
            source: Rc::new(src.into()),
        }
    }
    pub fn root(&self) -> Node {
        Node {
            inner: self.inner.root_node(),
            source: &self.source,
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
