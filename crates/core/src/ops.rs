use crate::matcher::{MatchAll, MatchNone, Matcher, PositiveMatcher};
use crate::meta_var::{MetaVarEnv, MetaVarMatcher, MetaVarMatchers};
use crate::Language;
use crate::Node;
use std::marker::PhantomData;

pub struct And<L: Language, P1: Matcher<L>, P2: Matcher<L>> {
    pattern1: P1,
    pattern2: P2,
    lang: PhantomData<L>,
}

impl<L: Language, P1, P2> PositiveMatcher<L> for And<L, P1, P2>
where
    P1: PositiveMatcher<L>,
    P2: Matcher<L>,
{
}

impl<L: Language, P1, P2> Matcher<L> for And<L, P1, P2>
where
    P1: Matcher<L>,
    P2: Matcher<L>,
{
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let node = self.pattern1.match_node_with_env(node, env)?;
        self.pattern2.match_node_with_env(node, env)
    }
}

pub struct All<L: Language, P: Matcher<L>> {
    patterns: Vec<P>,
    lang: PhantomData<L>,
}

impl<L: Language, P: Matcher<L>> All<L, P> {
    pub fn new<PS: IntoIterator<Item = P>>(patterns: PS) -> Self {
        Self {
            patterns: patterns.into_iter().collect(),
            lang: PhantomData,
        }
    }
}

impl<L: Language, P: Matcher<L>> Matcher<L> for All<L, P> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.patterns
            .iter()
            .all(|p| p.match_node_with_env(node, env).is_some())
            .then_some(node)
    }
}

// TODO: this is not correct. We don't need every sub pattern to be positive
// need to refine this
impl<L: Language, M: Matcher<L>> PositiveMatcher<L> for All<L, M> {}

pub struct Any<L, P> {
    patterns: Vec<P>,
    lang: PhantomData<L>,
}

impl<L: Language, P: Matcher<L>> Any<L, P> {
    pub fn new<PS: IntoIterator<Item = P>>(patterns: PS) -> Self {
        Self {
            patterns: patterns.into_iter().collect(),
            lang: PhantomData,
        }
    }
}

impl<L: Language, M: Matcher<L>> Matcher<L> for Any<L, M> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.patterns
            .iter()
            .find_map(|p| p.match_node_with_env(node, env))
            .map(|_| node)
    }
}

impl<L: Language, P: PositiveMatcher<L>> PositiveMatcher<L> for Any<L, P> {}

pub struct Or<L: Language, P1: Matcher<L>, P2: Matcher<L>> {
    pattern1: P1,
    pattern2: P2,
    lang: PhantomData<L>,
}

impl<L, P1, P2> Matcher<L> for Or<L, P1, P2>
where
    L: Language,
    P1: Matcher<L>,
    P2: Matcher<L>,
{
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.pattern1
            .match_node_with_env(node, env)
            .or_else(|| self.pattern2.match_node_with_env(node, env))
    }
}

impl<L, P1, P2> PositiveMatcher<L> for Or<L, P1, P2>
where
    L: Language,
    P1: PositiveMatcher<L>,
    P2: PositiveMatcher<L>,
{
}

pub struct Not<L: Language, M: Matcher<L>> {
    not: M,
    lang: PhantomData<L>,
}

impl<L: Language, M: Matcher<L>> Not<L, M> {
    pub fn new(not: M) -> Self {
        Self {
            not,
            lang: PhantomData,
        }
    }
}
impl<L, P> Matcher<L> for Not<L, P>
where
    L: Language,
    P: Matcher<L>,
{
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.not.match_node_with_env(node, env).xor(Some(node))
    }
}

#[derive(Clone)]
pub struct Op<L: Language, M: Matcher<L>> {
    inner: M,
    meta_vars: MetaVarMatchers<L>,
}

impl<L, M> Matcher<L> for Op<L, M>
where
    L: Language,
    M: Matcher<L>,
{
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.inner.match_node_with_env(node, env)
    }

    fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
        // TODO: avoid clone here
        self.meta_vars.clone()
    }
}
impl<L, P> PositiveMatcher<L> for Op<L, P>
where
    L: Language,
    P: PositiveMatcher<L>,
{
}

impl<L: Language, M: Matcher<L>> Op<L, M> {
    pub fn not(pattern: M) -> Not<L, M> {
        Not {
            not: pattern,
            lang: PhantomData,
        }
    }

    pub fn with_meta_var(&mut self, var_id: String, matcher: MetaVarMatcher<L>) -> &mut Self {
        self.meta_vars.insert(var_id, matcher);
        self
    }
}

impl<L: Language, M: PositiveMatcher<L>> Op<L, M> {
    pub fn every(pattern: M) -> Op<L, And<L, M, MatchAll>> {
        Op {
            inner: And {
                pattern1: pattern,
                pattern2: MatchAll,
                lang: PhantomData,
            },
            meta_vars: MetaVarMatchers::new(),
        }
    }
    pub fn either(pattern: M) -> Op<L, Or<L, M, MatchNone>> {
        Op {
            inner: Or {
                pattern1: pattern,
                pattern2: MatchNone,
                lang: PhantomData,
            },
            meta_vars: MetaVarMatchers::new(),
        }
    }

    pub fn new(matcher: M) -> Op<L, M> {
        Self {
            inner: matcher,
            meta_vars: MetaVarMatchers::new(),
        }
    }
}

impl<L: Language, M: Matcher<L>, N: Matcher<L>> Op<L, And<L, M, N>> {
    pub fn and<O: Matcher<L>>(self, other: O) -> Op<L, And<L, And<L, M, N>, O>> {
        Op {
            inner: And {
                pattern1: self.inner,
                pattern2: other,
                lang: PhantomData,
            },
            meta_vars: self.meta_vars,
        }
    }
}

impl<L: Language, M: Matcher<L>, N: Matcher<L>> Op<L, Or<L, M, N>> {
    pub fn or<O: Matcher<L>>(self, other: O) -> Op<L, Or<L, Or<L, M, N>, O>> {
        Op {
            inner: Or {
                pattern1: self.inner,
                pattern2: other,
                lang: PhantomData,
            },
            meta_vars: self.meta_vars,
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::language::Tsx;
    use crate::Root;

    fn test_find(matcher: &impl Matcher<Tsx>, code: &str) {
        let node = Root::new(code, Tsx);
        assert!(matcher.find_node(node.root()).is_some());
    }
    fn test_not_find(matcher: &impl Matcher<Tsx>, code: &str) {
        let node = Root::new(code, Tsx);
        assert!(matcher.find_node(node.root()).is_none());
    }
    fn find_all(matcher: impl Matcher<Tsx>, code: &str) -> Vec<String> {
        let node = Root::new(code, Tsx);
        matcher
            .find_all_nodes(node.root())
            .map(|n| n.text().to_string())
            .collect()
    }

    #[test]
    fn test_or() {
        let matcher = Or {
            pattern1: "let a = 1",
            pattern2: "const b = 2",
            lang: PhantomData,
        };
        test_find(&matcher, "let a = 1");
        test_find(&matcher, "const b = 2");
        test_not_find(&matcher, "let a = 2");
        test_not_find(&matcher, "const a = 1");
        test_not_find(&matcher, "let b = 2");
        test_not_find(&matcher, "const b = 1");
    }

    #[test]
    fn test_not() {
        let matcher = Not {
            not: "let a = 1",
            lang: PhantomData,
        };
        test_find(&matcher, "const b = 2");
    }

    #[test]
    fn test_and() {
        let matcher = And {
            pattern1: "let a = $_",
            pattern2: Not {
                not: "let a = 123",
                lang: PhantomData,
            },
            lang: PhantomData,
        };
        test_find(&matcher, "let a = 233");
        test_find(&matcher, "let a = 456");
        test_not_find(&matcher, "let a = 123");
    }

    #[test]
    fn test_api_and() {
        let matcher = Op::every("let a = $_").and(Op::not("let a = 123"));
        test_find(&matcher, "let a = 233");
        test_find(&matcher, "let a = 456");
        test_not_find(&matcher, "let a = 123");
    }

    #[test]
    fn test_api_or() {
        let matcher = Op::either("let a = 1").or("const b = 2");
        test_find(&matcher, "let a = 1");
        test_find(&matcher, "const b = 2");
        test_not_find(&matcher, "let a = 2");
        test_not_find(&matcher, "const a = 1");
        test_not_find(&matcher, "let b = 2");
        test_not_find(&matcher, "const b = 1");
    }
    #[test]
    fn test_multiple_match() {
        let sequential = find_all("$A + b", "let f = () => a + b; let ff = () => c + b");
        assert_eq!(sequential.len(), 2);
        let nested = find_all(
            "function $A() { $$$ }",
            "function a() { function b() { b } }",
        );
        assert_eq!(nested.len(), 2);
    }

    #[test]
    fn test_multiple_match_order() {
        let ret = find_all(
            "$A + b",
            "let f = () => () => () => a + b; let ff = () => c + b",
        );
        assert_eq!(ret, ["a + b", "c + b"], "should match source code order");
    }
}
