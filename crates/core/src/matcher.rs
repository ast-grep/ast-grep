use crate::meta_var::{MetaVarEnv, MetaVarMatchers};
use crate::node::{KindId, DFS};
use crate::Language;
use crate::Node;
use crate::Pattern;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct KindMatcher<L: Language> {
    kind: KindId,
    lang: PhantomData<L>,
}

impl<L: Language> KindMatcher<L> {
    pub fn new(node_kind: &str, lang: L) -> Self {
        Self {
            kind: lang
                .get_ts_language()
                .id_for_node_kind(node_kind, /*named*/ true),
            lang: PhantomData,
        }
    }
}

impl<L: Language> Matcher<L> for KindMatcher<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        _env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        if node.kind_id() == self.kind {
            Some(node)
        } else {
            None
        }
    }
}
impl<L: Language> PositiveMatcher<L> for KindMatcher<L> {}

/**
 * N.B. At least one positive term is required for matching
 */
pub trait Matcher<L: Language> {
    fn match_node_with_env<'tree>(
        &self,
        _node: Node<'tree, L>,
        _env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>>;

    fn match_node<'tree>(&self, node: Node<'tree, L>) -> Option<Node<'tree, L>> {
        let mut env = self.get_meta_var_env();
        let node = self.match_node_with_env(node, &mut env)?;
        env.match_constraints().then_some(node)
    }

    fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
        MetaVarMatchers::new()
    }

    fn get_meta_var_env<'tree>(&self) -> MetaVarEnv<'tree, L> {
        MetaVarEnv::from_matchers(self.get_meta_var_matchers())
    }

    fn find_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.match_node_with_env(node, env).or_else(|| {
            node.children()
                .find_map(|sub| self.find_node_with_env(sub, env))
        })
    }

    fn find_node<'tree>(&self, node: Node<'tree, L>) -> Option<Node<'tree, L>> {
        self.match_node(node)
            .or_else(|| node.children().find_map(|sub| self.find_node(sub)))
    }

    fn find_all_nodes<'tree>(self, node: Node<'tree, L>) -> FindAllNodes<'tree, L, Self> where Self: Sized {
        FindAllNodes::new(self, node)
    }
}

impl<S: AsRef<str>, L: Language> Matcher<L> for S {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let pattern = Pattern::new(self.as_ref(), node.root.lang);
        pattern.match_node_with_env(node, env)
    }
}

impl<S: AsRef<str>, L: Language> PositiveMatcher<L> for S {}

impl<L: Language> Matcher<L> for Box<dyn Matcher<L>> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        // NOTE: must double deref boxed value to avoid recursion
        (**self).match_node_with_env(node, env)
    }
}
impl<L: Language> Matcher<L> for Box<dyn PositiveMatcher<L>> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        // NOTE: must double deref boxed value to avoid recursion
        (**self).match_node_with_env(node, env)
    }
}
impl<L: Language> PositiveMatcher<L> for Box<dyn PositiveMatcher<L>> {
}

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
        Self {
            dfs: node.dfs(),
            matcher,
        }
    }
}

impl<'tree, L: Language, M: Matcher<L>> Iterator for FindAllNodes<'tree, L, M> {
    type Item = Node<'tree, L>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(cand) = self.dfs.next() {
            if let Some(matched) = self.matcher.match_node(cand) {
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
        let pattern = KindMatcher::new(kind, Tsx);
        assert!(
            pattern.find_node(cand).is_some(),
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
        let pattern = KindMatcher::new(kind, Tsx);
        assert!(
            pattern.find_node(cand).is_none(),
            "goal: {}, candidate: {}",
            kind,
            cand.inner.to_sexp(),
        );
    }

    #[test]
    fn test_box_match() {
        let boxed: Box<dyn Matcher<Tsx>> = Box::new("const a = 123");
        let cand = pattern_node("const a = 123");
        let cand = cand.root();
        assert!(
            boxed.find_node(cand).is_some()
        );
    }
}
