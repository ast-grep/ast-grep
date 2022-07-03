use crate::meta_var::MetaVarEnv;
use crate::Node;
use crate::Pattern;
use std::collections::VecDeque;

/**
 * N.B. At least one positive term is required for matching
 */
pub trait Matcher {
    fn match_node<'tree>(&self, _node: Node<'tree>, _env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>>;

    fn find_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        self.match_node(node, env).or_else(||
            node.children().find_map(|sub| self.find_node(sub, env))
        )
    }

    fn find_node_vec<'tree>(&self, node: Node<'tree>) -> Vec<Node<'tree>> {
        let mut ret = vec![];
        let mut queue = VecDeque::new();
        queue.push_back(node);
        while let Some(cand) = queue.pop_front() {
            queue.extend(cand.children());
            let mut env = MetaVarEnv::new();
            if let Some(matched) = self.match_node(cand, &mut env) {
                ret.push(matched);
            }
        }
        ret
    }
}

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
    P2: Matcher
{}


impl<P1, P2> Matcher for And<P1, P2>
where
    P1: Matcher,
    P2: Matcher,
{
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        let node = self.pattern1.match_node(node, env)?;
        self.pattern2.match_node(node, env)
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
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        self.pattern1.match_node(node, env).or_else(|| self.pattern2.match_node(node, env))
    }
}

impl<P1, P2> PositiveMatcher for Or<P1, P2>
where
    P1: PositiveMatcher,
    P2: PositiveMatcher,
{}

pub struct Inside {
    outer: Pattern,
}

impl Matcher for Inside {
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        let mut n = node;
        while let Some(p) = n.parent() {
            if self.outer.match_node(p, env).is_some() {
                return Some(node)
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
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        let mut n = node;
        while let Some(p) = n.parent() {
            if self.outer.match_node(p, env).is_some() {
                return None
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
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        if self.not.match_node(node, env).is_none() {
            Some(node)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Pattern;
    use crate::Root;
    fn test_find(rule: &impl Matcher, code: &str) {
        let mut env = MetaVarEnv::new();
        let node = Root::new(code);
        assert!(rule.find_node(node.root(), &mut env).is_some());
    }
    fn test_not_find(rule: &impl Matcher, code: &str) {
        let mut env = MetaVarEnv::new();
        let node = Root::new(code);
        assert!(rule.find_node(node.root(), &mut env).is_none());
    }

    #[test]
    fn test_or() {
        let rule = Or {
            pattern1: Pattern::new("let a = 1"),
            pattern2: Pattern::new("const b = 2"),
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
            not: Pattern::new("let a = 1"),
        };
        test_find(&rule, "const b = 2");
    }

    #[test]
    fn test_and() {
        let rule = And {
            pattern1: Pattern::new("let a = $_"),
            pattern2: Not { not: Pattern::new("let a = 123") }
        };
        test_find(&rule, "let a = 233");
        test_find(&rule, "let a = 456");
        test_not_find(&rule, "let a = 123");
    }
}
