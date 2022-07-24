use crate::Language;
use crate::Node;
use crate::Pattern;
use crate::meta_var::{MetaVarMatchers, MetaVarEnv, MetaVarMatcher};
use std::marker::PhantomData;
use crate::matcher::{Matcher, PositiveMatcher};

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

pub struct Either<P> {
    patterns: Vec<P>,
}

impl<L: Language, P: Matcher<L>> Matcher<L> for Either<P> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.patterns
            .iter()
            .any(|p| p.match_node_with_env(node, env).is_some())
            .then_some(node)
    }
}

pub struct Or<L: Language, P1: PositiveMatcher<L>, P2: PositiveMatcher<L>> {
    pattern1: P1,
    pattern2: P2,
    lang: PhantomData<L>,
}

impl<L, P1, P2> Matcher<L> for Or<L, P1, P2>
where
    L: Language,
    P1: PositiveMatcher<L>,
    P2: PositiveMatcher<L>,
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

pub struct Inside<L: Language> {
    outer: Pattern<L>,
}

impl<L: Language> Matcher<L> for Inside<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let mut n = node;
        while let Some(p) = n.parent() {
            if self.outer.match_node_with_env(p, env).is_some() {
                return Some(node);
            }
            n = p;
        }
        None
    }
}

pub struct NotInside<L: Language> {
    outer: Pattern<L>,
}

impl<L: Language> Matcher<L> for NotInside<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let mut n = node;
        while let Some(p) = n.parent() {
            if self.outer.match_node_with_env(p, env).is_some() {
                return None;
            }
            n = p;
        }
        Some(node)
    }
}

pub struct Not<L: Language, P: PositiveMatcher<L>> {
    not: P,
    lang: PhantomData<L>,
}

impl<L, P> Matcher<L> for Not<L, P>
where
    L: Language,
    P: PositiveMatcher<L>,
{
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.not.match_node_with_env(node, env).xor(Some(node))
    }
}

pub struct Rule<L: Language, M: Matcher<L>> {
    inner: M,
    meta_vars: MetaVarMatchers<L>,
}

impl<L, M> Matcher<L> for Rule<L, M>
where
    L: Language,
    M: Matcher<L>,
{
    fn match_node_with_env<'tree>(&self, node: Node<'tree, L>, env: &mut MetaVarEnv<'tree, L>) -> Option<Node<'tree, L>> {
        self.inner.match_node_with_env(node, env)
    }

    fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
        // TODO: avoid clone here
        self.meta_vars.clone()
    }
}
impl<L, P> PositiveMatcher<L> for Rule<L, P>
where
    L: Language,
    P: PositiveMatcher<L> {}

impl<L: Language, M: PositiveMatcher<L>> Rule<L, M> {
    pub fn all(pattern: M) -> AndRule<L, M> {
        AndRule {
            inner: pattern.into(),
            lang: PhantomData,
        }
    }
    pub fn either(pattern: M) -> EitherRule<L, M> {
        EitherRule {
            inner: pattern,
            lang: PhantomData,
        }
    }
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

pub struct AndRule<L: Language, M: PositiveMatcher<L>> {
    inner: M,
    lang: PhantomData<L>,
}
impl<L: Language, M: PositiveMatcher<L>> AndRule<L, M> {
    pub fn and<N: Matcher<L>>(self, other: N) -> Rule<L, And<L, M, N>> {
        Rule {
            inner: And {
                pattern1: self.inner,
                pattern2: other,
                lang: PhantomData,
            },
            meta_vars: MetaVarMatchers::new(),
        }
    }
}
impl<L: Language, M: PositiveMatcher<L>, N: Matcher<L>> Rule<L, And<L, M, N>> {
    pub fn and<O: Matcher<L>>(self, other: O) -> Rule<L, And<L, And<L, M, N>, O>> {
        Rule {
            inner: And {
                pattern1: self.inner,
                pattern2: other,
                lang: PhantomData,
            },
            meta_vars: self.meta_vars,
        }
    }
}

pub struct EitherRule<L: Language, M: PositiveMatcher<L>> {
    inner: M,
    lang: PhantomData<L>,
}
impl<L: Language, M: PositiveMatcher<L>> EitherRule<L, M> {
    pub fn or<N: PositiveMatcher<L>>(self, other: N) -> Rule<L, Or<L, M, N>> {
        Rule {
            inner: Or {
                pattern1: self.inner,
                pattern2: other,
                lang: PhantomData,
            },
            meta_vars: MetaVarMatchers::new(),
        }
    }
}

impl<L: Language, M: PositiveMatcher<L>, N: PositiveMatcher<L>> Rule<L, Or<L, M, N>> {
    pub fn or<O: PositiveMatcher<L>>(self, other: O) -> Rule<L, Or<L, Or<L, M, N>, O>> {
        Rule {
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

    fn test_find(rule: &impl Matcher<Tsx>, code: &str) {
        let node = Root::new(code, Tsx);
        assert!(rule.find_node(node.root()).is_some());
    }
    fn test_not_find(rule: &impl Matcher<Tsx>, code: &str) {
        let node = Root::new(code, Tsx);
        assert!(rule.find_node(node.root()).is_none());
    }
    fn find_all(rule: impl Matcher<Tsx>, code: &str) -> Vec<String> {
        let node = Root::new(code, Tsx);
        rule.find_all_nodes(node.root()).map(|n| n.text().to_string()).collect()
    }

    #[test]
    fn test_or() {
        let rule = Or {
            pattern1: "let a = 1",
            pattern2: "const b = 2",
            lang: PhantomData,
        };
        test_find(&rule, "let a = 1");
        test_find(&rule, "const b = 2");
        test_not_find(&rule, "let a = 2");
        test_not_find(&rule, "const a = 1");
        test_not_find(&rule, "let b = 2");
        test_not_find(&rule, "const b = 1");
    }

    #[test]
    fn test_not() {
        let rule = Not {
            not: "let a = 1",
            lang: PhantomData,
        };
        test_find(&rule, "const b = 2");
    }

    #[test]
    fn test_and() {
        let rule = And {
            pattern1: "let a = $_",
            pattern2: Not {
                not: "let a = 123",
                lang: PhantomData,
            },
            lang: PhantomData,
        };
        test_find(&rule, "let a = 233");
        test_find(&rule, "let a = 456");
        test_not_find(&rule, "let a = 123");
    }

    #[test]
    fn test_api_and() {
        let rule = Rule::all("let a = $_")
            .and(Rule::not("let a = 123"));
        test_find(&rule, "let a = 233");
        test_find(&rule, "let a = 456");
        test_not_find(&rule, "let a = 123");
    }

    #[test]
    fn test_api_or() {
        let rule = Rule::either("let a = 1").or("const b = 2");
        test_find(&rule, "let a = 1");
        test_find(&rule, "const b = 2");
        test_not_find(&rule, "let a = 2");
        test_not_find(&rule, "const a = 1");
        test_not_find(&rule, "let b = 2");
        test_not_find(&rule, "const b = 1");
    }
    #[test]
    fn test_multiple_match() {
        let sequential = find_all("$A + b", "let f = () => a + b; let ff = () => c + b");
        assert_eq!(sequential.len(), 2);
        let nested = find_all("function $A() { $$$ }", "function a() { function b() { b } }");
        assert_eq!(nested.len(), 2);
    }

    #[test]
    fn test_multiple_match_order() {
        let ret = find_all("$A + b", "let f = () => () => () => a + b; let ff = () => c + b");
        assert_eq!(ret, ["a + b", "c + b"], "should match source code order");
    }
}
