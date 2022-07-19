use crate::meta_var::MetaVarEnv;
use crate::Node;
use crate::Language;
use crate::Pattern;
use crate::node::DFS;

/**
 * N.B. At least one positive term is required for matching
 */
pub trait Matcher<L: Language>: Sized {
    fn match_node<'tree>(
        &self,
        _node: Node<'tree, L>,
        _env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>>;

    fn find_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.match_node(node, env)
            .or_else(|| node.children().find_map(|sub| self.find_node(sub, env)))
    }

    fn find_all_nodes<'tree>(self, node: Node<'tree, L>) -> FindAllNodes<'tree, L, Self> {
        FindAllNodes::new(self, node)
    }
}

impl<S: AsRef<str>, L: Language> Matcher<L> for S {
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let pattern = Pattern::new(self.as_ref(), node.root.lang);
        pattern.match_node(node, env)
    }
}

impl<S: AsRef<str>, L: Language> PositiveMatcher<L> for S {}

/**
 * A marker trait to indicate the the rule is positive matcher
 */
pub trait PositiveMatcher<L: Language>: Matcher<L> {}


pub struct FindAllNodes<'tree, L: Language, M: Matcher<L>> {
    dfs: DFS<'tree, L>,
    matcher: M,
}

impl<'tree, L: Language, M: Matcher<L>> FindAllNodes<'tree, L, M> {
    fn new(matcher: M, node: Node<'tree, L>) -> Self {
        Self { dfs: node.dfs(), matcher }
    }
}

impl<'tree, L: Language, M: Matcher<L>> Iterator for FindAllNodes<'tree, L, M> {
    type Item = Node<'tree, L>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(cand) = self.dfs.next() {
            let mut env = MetaVarEnv::new();
            if let Some(matched) = self.matcher.match_node(cand, &mut env) {
                return Some(matched);
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::language::Tsx;
    use crate::Root;

    fn pattern_node(s: &str) -> Root<Tsx> {
        Root::new(s, Tsx)
    }
    #[test]
    fn test_kind_match() {
        let kind = "public_field_definition";
        let cand = pattern_node("class A { a = 123 }");
        let cand = cand.root();
        let pattern = Pattern::of_kind(kind);
        let mut env = MetaVarEnv::new();
        assert!(
            pattern.find_node(cand, &mut env).is_some(),
            "goal: {}, candidate: {}",
            kind,
            cand.inner.to_sexp(),
        );
    }

    #[test]
    fn test_kind_non_match() {
        let kind = "field_definition";
        let cand = pattern_node("const a = 123");
        let cand = cand.root();
        let pattern = Pattern::of_kind(kind);
        let mut env = MetaVarEnv::new();
        assert!(
            pattern.find_node(cand, &mut env).is_none(),
            "goal: {}, candidate: {}",
            kind,
            cand.inner.to_sexp(),
        );
    }
}
