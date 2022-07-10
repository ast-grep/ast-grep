use crate::meta_var::MetaVarEnv;
use crate::replacer::Replacer;
use crate::rule::Matcher;
use crate::ts_parser::Edit;

// the lifetime r represents root
#[derive(Clone, Copy)]
pub struct Node<'r> {
    pub(crate) inner: tree_sitter::Node<'r>,
    pub(crate) source: &'r str,
}
type NodeKind = u16;

struct NodeWalker<'tree> {
    cursor: tree_sitter::TreeCursor<'tree>,
    source: &'tree str,
    count: usize,
}

impl<'tree> Iterator for NodeWalker<'tree> {
    type Item = Node<'tree>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let ret = Some(Node {
            inner: self.cursor.node(),
            source: self.source,
        });
        self.cursor.goto_next_sibling();
        self.count -= 1;
        ret
    }
}

impl<'tree> ExactSizeIterator for NodeWalker<'tree> {
    fn len(&self) -> usize {
        self.count
    }
}

// internal API
impl<'r> Node<'r> {
    pub fn is_leaf(&self) -> bool {
        self.inner.child_count() == 0
    }
    pub fn kind(&self) -> &str {
        self.inner.kind()
    }
    pub fn kind_id(&self) -> NodeKind {
        self.inner.kind_id()
    }
    pub fn text(&self) -> &'r str {
        self.inner
            .utf8_text(self.source.as_bytes())
            .expect("invalid source text encoding")
    }

    pub fn children(&self) -> impl ExactSizeIterator<Item = Node<'r>> + '_ {
        let mut cursor = self.inner.walk();
        cursor.goto_first_child();
        NodeWalker {
            cursor,
            source: self.source,
            count: self.inner.child_count(),
        }
    }

    pub fn display_context(&self) -> DisplayContext<'r> {
        let bytes = self.source.as_bytes();
        let start = self.inner.start_byte();
        let end = self.inner.end_byte();
        let (mut leading, mut trailing) = (start, end);
        while leading > 0 && bytes[leading - 1] != b'\n' {
            leading -= 1;
        }
        while trailing < bytes.len() - 1 && bytes[trailing + 1] != b'\n' {
            trailing += 1;
        }
        DisplayContext {
            matched: self.text(),
            leading: &self.source[leading..start],
            trailing: &self.source[end..=trailing],
            start_line: self.inner.start_position().row + 1,
        }
    }
}

pub struct DisplayContext<'r> {
    /// content for the matched node
    pub matched: &'r str,
    /// content before the matched node
    pub leading: &'r str,
    /// content after the matched node
    pub trailing: &'r str,
    /// start line of the matched node
    pub start_line: usize,
}

// tree traversal API
impl<'r> Node<'r> {
    #[must_use]
    pub fn find<M: Matcher>(&self, pat: M) -> Option<Node<'r>> {
        let mut env = MetaVarEnv::new();
        pat.find_node(*self, &mut env)
    }

    pub fn find_all<M: Matcher>(&self, pat: M) -> impl Iterator<Item=Node<'r>> {
        pat.find_all_nodes(*self)
    }

    // should we provide parent?
    #[must_use]
    pub fn parent(&self) -> Option<Node<'r>> {
        let inner = self.inner.parent()?;
        Some(Node {
            inner,
            source: self.source,
        })
    }
    pub fn ancestors(&self) -> impl Iterator<Item = Node<'r>> + '_ {
        let mut parent = self.inner.parent();
        std::iter::from_fn(move || {
            let inner = parent?;
            let ret = Some(Node {
                inner,
                source: self.source,
            });
            parent = inner.parent();
            ret
        })
    }
    #[must_use]
    pub fn next(&self) -> Option<Node<'r>> {
        let inner = self.inner.next_sibling()?;
        Some(Node {
            inner,
            source: self.source,
        })
    }
    pub fn next_all(&self) -> impl Iterator<Item = Node<'r>> + '_ {
        let mut cursor = self.inner.walk();
        let source = self.source;
        std::iter::from_fn(move || {
            if cursor.goto_next_sibling() {
                Some(Node {
                    inner: cursor.node(),
                    source,
                })
            } else {
                None
            }
        })
    }
    #[must_use]
    pub fn prev(&self) -> Option<Node<'r>> {
        let inner = self.inner.prev_sibling()?;
        Some(Node {
            inner,
            source: self.source,
        })
    }
    #[must_use]
    pub fn eq(&self, _i: usize) -> Node<'r> {
        todo!()
    }
    pub fn each<F>(&self, _f: F)
    where
        F: Fn(&Node<'r>),
    {
        todo!()
    }
}

// r manipulation API
impl<'r> Node<'r> {
    pub fn attr(&mut self) {}
    pub fn replace<M: Matcher, R: Replacer>(&mut self, matcher: M, replacer: R) -> Option<Edit> {
        let mut env = MetaVarEnv::new();
        let node = matcher.find_node(*self, &mut env)?;
        let inner = node.inner;
        let position = inner.start_byte();
        // instead of using start_byte/end_byte, ignore trivia like semicolon ;
        let named_cnt = inner.named_child_count();
        let end = inner.named_child(named_cnt - 1).unwrap().end_byte();
        let deleted_length = end - position;
        let inserted_text = replacer.generate_replacement(&env);
        Some(Edit {
            position,
            deleted_length,
            inserted_text,
        })
    }
    pub fn replace_by(&mut self) {}
    pub fn after(&mut self) {}
    pub fn before(&mut self) {}
    pub fn append(&mut self) {}
    pub fn prepend(&mut self) {}
    pub fn empty(&mut self) {}
    pub fn remove(&mut self) {}
    pub fn clone(&mut self) {}
}

impl<'r> std::fmt::Display for Node<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text())
    }
}

impl<'r> std::fmt::Debug for Node<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text())
    }
}

#[cfg(test)]
mod test {
    use crate::Root;
    #[test]
    fn test_is_leaf() {
        let root = Root::new("let a = 123");
        let node = root.root();
        assert!(!node.is_leaf());
    }

    #[test]
    fn test_children() {
        let root = Root::new("let a = 123");
        let node = root.root();
        let children: Vec<_> = node.children().collect();
        assert_eq!(children.len(), 1);
        let texts: Vec<_> = children[0]
            .children()
            .map(|c| c.text().to_string())
            .collect();
        assert_eq!(texts, vec!["let", "a = 123"]);
    }
}
