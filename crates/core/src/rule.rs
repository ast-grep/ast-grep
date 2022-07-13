use crate::meta_var::MetaVarEnv;
use crate::Language;
use crate::Node;
use crate::Pattern;
use std::collections::VecDeque;
use std::marker::PhantomData;

pub struct FindAllNodes<'tree, L: Language, M: Matcher<L>> {
    queue: VecDeque<Node<'tree, L>>,
    matcher: M,
}

impl<'tree, L: Language, M: Matcher<L>> FindAllNodes<'tree, L, M> {
    fn new(matcher: M, node: Node<'tree, L>) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(node);
        Self { queue, matcher }
    }
}

impl<'tree, L: Language, M: Matcher<L>> Iterator for FindAllNodes<'tree, L, M> {
    type Item = Node<'tree, L>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(cand) = self.queue.pop_front() {
            self.queue.extend(cand.children());
            let mut env = MetaVarEnv::new();
            if let Some(matched) = self.matcher.match_node(cand, &mut env) {
                return Some(matched);
            }
        }
        None
    }
}

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

    fn find_all_nodes<'tree>(self, node: Node<'tree, L>) -> Box<dyn Iterator<Item = Node<'tree, L>> + 'tree>
    where Self: 'static {
        // TODO: remove the Box here
        Box::new(node.dfs().filter_map(move |node| {
            let mut env = MetaVarEnv::new();
            self.match_node(node, &mut env)
        }))
        // FindAllNodes::new(self, node)
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
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let node = self.pattern1.match_node(node, env)?;
        self.pattern2.match_node(node, env)
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
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.patterns
            .iter()
            .all(|p| p.match_node(node, env).is_some())
            .then_some(node)
    }
}

pub struct Either<P> {
    patterns: Vec<P>,
}

impl<L: Language, P: Matcher<L>> Matcher<L> for Either<P> {
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.patterns
            .iter()
            .any(|p| p.match_node(node, env).is_some())
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
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.pattern1
            .match_node(node, env)
            .or_else(|| self.pattern2.match_node(node, env))
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
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let mut n = node;
        while let Some(p) = n.parent() {
            if self.outer.match_node(p, env).is_some() {
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
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        let mut n = node;
        while let Some(p) = n.parent() {
            if self.outer.match_node(p, env).is_some() {
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
    fn match_node<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.not.match_node(node, env).xor(Some(node))
    }
}

pub struct Rule<L: Language, M: Matcher<L>> {
    inner: M,
    lang: PhantomData<L>,
}

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
    pub fn build(self) -> M {
        self.inner
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
            lang: PhantomData,
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
            lang: PhantomData,
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
            lang: PhantomData,
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
            lang: PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::language::Tsx;
    use crate::Root;

    fn test_find(rule: &impl Matcher<Tsx>, code: &str) {
        let mut env = MetaVarEnv::new();
        let node = Root::new(code, Tsx);
        assert!(rule.find_node(node.root(), &mut env).is_some());
    }
    fn test_not_find(rule: &impl Matcher<Tsx>, code: &str) {
        let mut env = MetaVarEnv::new();
        let node = Root::new(code, Tsx);
        assert!(rule.find_node(node.root(), &mut env).is_none());
    }
    fn find_all(rule: impl Matcher<Tsx> + 'static, code: &str) -> Vec<String> {
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
            .and(Rule::not("let a = 123"))
            .build();
        test_find(&rule, "let a = 233");
        test_find(&rule, "let a = 456");
        test_not_find(&rule, "let a = 123");
    }

    #[test]
    fn test_api_or() {
        let rule = Rule::either("let a = 1").or("const b = 2").build();
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
