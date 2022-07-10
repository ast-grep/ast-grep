use crate::meta_var::MetaVarEnv;
use crate::Node;
use crate::Pattern;
use std::collections::VecDeque;

pub struct FindAllNodes<'tree, M: Matcher> {
    queue: VecDeque<Node<'tree>>,
    matcher: M,
}

impl<'tree, M: Matcher> FindAllNodes<'tree, M> {
    fn new(matcher: M, node: Node<'tree>) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(node);
        Self {
            queue,
            matcher,
        }
    }
}

impl<'tree, M: Matcher> Iterator for FindAllNodes<'tree, M> {
    type Item = Node<'tree>;
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
pub trait Matcher: Sized {
    fn match_node<'tree>(
        &self,
        _node: Node<'tree>,
        _env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>>;

    fn find_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        self.match_node(node, env)
            .or_else(|| node.children().find_map(|sub| self.find_node(sub, env)))
    }

    fn find_all_nodes<'tree>(self, node: Node<'tree>) -> FindAllNodes<'tree, Self> {
        FindAllNodes::new(self, node)
    }
}

impl<S: AsRef<str>> Matcher for S {
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        // TODO: replace this
        let pattern = Pattern::new(self.as_ref(), tree_sitter_typescript::language_tsx());
        pattern.match_node(node, env)
    }
}

impl<S: AsRef<str>> PositiveMatcher for S {}

/**
 * A marker trait to indicate the the rule is positive matcher
 */
pub trait PositiveMatcher: Matcher {}

pub struct And<P1: Matcher, P2: Matcher> {
    pattern1: P1,
    pattern2: P2,
}

impl<P1, P2> PositiveMatcher for And<P1, P2>
where
    P1: PositiveMatcher,
    P2: Matcher,
{
}

impl<P1, P2> Matcher for And<P1, P2>
where
    P1: Matcher,
    P2: Matcher,
{
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        let node = self.pattern1.match_node(node, env)?;
        self.pattern2.match_node(node, env)
    }
}

pub struct All<P: Matcher> {
    patterns: Vec<P>,
}

impl<P: Matcher> All<P> {
    pub fn new<PS: IntoIterator<Item=P>>(patterns: PS) -> Self {
        Self {
            patterns: patterns.into_iter().collect(),
        }
    }
}

impl<P: Matcher> Matcher for All<P> {
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        self.patterns
            .iter()
            .all(|p| p.match_node(node, env).is_some())
            .then_some(node)
    }
}

pub struct Either<P: Matcher> {
    patterns: Vec<P>,
}

impl<P: Matcher> Matcher for Either<P> {
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        self.patterns
            .iter()
            .any(|p| p.match_node(node, env).is_some())
            .then_some(node)
    }
}

pub struct Or<P1: PositiveMatcher, P2: PositiveMatcher> {
    pattern1: P1,
    pattern2: P2,
}

impl<P1, P2> Matcher for Or<P1, P2>
where
    P1: PositiveMatcher,
    P2: PositiveMatcher,
{
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        self.pattern1
            .match_node(node, env)
            .or_else(|| self.pattern2.match_node(node, env))
    }
}

impl<P1, P2> PositiveMatcher for Or<P1, P2>
where
    P1: PositiveMatcher,
    P2: PositiveMatcher,
{
}

pub struct Inside {
    outer: Pattern,
}

impl Matcher for Inside {
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
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

pub struct NotInside {
    outer: Pattern,
}

impl Matcher for NotInside {
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
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

pub struct Not<P: PositiveMatcher> {
    not: P,
}

impl<P> Matcher for Not<P>
where
    P: PositiveMatcher,
{
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        self.not.match_node(node, env).xor(Some(node))
    }
}

pub struct Rule<M: Matcher> {
    inner: M,
}

impl<M: PositiveMatcher> Rule<M> {
    pub fn all(pattern: M) -> AndRule<M> {
        AndRule {
            inner: pattern.into(),
        }
    }
    pub fn either(pattern: M) -> EitherRule<M> {
        EitherRule { inner: pattern }
    }
    pub fn not(pattern: M) -> Not<M> {
        Not { not: pattern }
    }
    pub fn build(self) -> M {
        self.inner
    }
}

pub struct AndRule<M> {
    inner: M,
}
impl<M: PositiveMatcher> AndRule<M> {
    pub fn and<N: Matcher>(self, other: N) -> Rule<And<M, N>> {
        Rule {
            inner: And {
                pattern1: self.inner,
                pattern2: other,
            },
        }
    }
}
impl<M: PositiveMatcher, N: Matcher> Rule<And<M, N>> {
    pub fn and<O: Matcher>(self, other: O) -> Rule<And<And<M, N>, O>> {
        Rule {
            inner: And {
                pattern1: self.inner,
                pattern2: other,
            },
        }
    }
}

pub struct EitherRule<M> {
    inner: M,
}
impl<M: PositiveMatcher> EitherRule<M> {
    pub fn or<N: PositiveMatcher>(self, other: N) -> Rule<Or<M, N>> {
        Rule {
            inner: Or {
                pattern1: self.inner,
                pattern2: other,
            },
        }
    }
}

impl<M: PositiveMatcher, N: PositiveMatcher> Rule<Or<M, N>> {
    pub fn or<O: PositiveMatcher>(self, other: O) -> Rule<Or<Or<M, N>, O>> {
        Rule {
            inner: Or {
                pattern1: self.inner,
                pattern2: other,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Pattern;
    use crate::Root;
    use crate::language::{Language, Tsx};

    fn get_pattern(src: &str) -> Pattern {
        Pattern::new(src, Tsx::get_ts_language())
    }
    fn test_find(rule: &impl Matcher, code: &str) {
        let mut env = MetaVarEnv::new();
        let node = Root::new(code, Tsx::get_ts_language());
        assert!(rule.find_node(node.root(), &mut env).is_some());
    }
    fn test_not_find(rule: &impl Matcher, code: &str) {
        let mut env = MetaVarEnv::new();
        let node = Root::new(code, Tsx::get_ts_language());
        assert!(rule.find_node(node.root(), &mut env).is_none());
    }

    #[test]
    fn test_or() {
        let rule = Or {
            pattern1: get_pattern("let a = 1"),
            pattern2: get_pattern("const b = 2"),
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
            not: get_pattern("let a = 1"),
        };
        test_find(&rule, "const b = 2");
    }

    #[test]
    fn test_and() {
        let rule = And {
            pattern1: get_pattern("let a = $_"),
            pattern2: Not {
                not: get_pattern("let a = 123"),
            },
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
}
