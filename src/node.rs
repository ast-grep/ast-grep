use super::Pattern;
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
    pub fn text(&self) -> &str {
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
}

// tree traversal API
impl<'r> Node<'r> {
    #[must_use]
    pub fn find<P: Into<Pattern>>(&self, pat: P) -> Option<Node<'r>> {
        let goal: Pattern = pat.into();
        goal.match_node(*self).map(|f| f.0)
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
    pub fn replace(&mut self, pattern_str: &str, replacement_str: &str) -> &mut Self {
        let _to_match = Pattern::new(pattern_str);
        let _to_replace = Pattern::new(replacement_str);
        todo!()
        // if let Some(_node) = to_match.match_node(self) {
        //     todo!("change node content with replaced")
        // } else {
        //     todo!()
        // }
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
