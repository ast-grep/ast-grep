use crate::meta_var::MetaVarEnv;
use crate::Node;
use crate::Pattern;
use std::collections::VecDeque;

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

pub struct And<P1: Matcher, P2: Matcher> {
    pattern1: P1,
    pattern2: P2,
}

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

pub struct Or<P1: Matcher, P2: Matcher> {
    pattern1: P1,
    pattern2: P2,
}

impl<P1, P2> Matcher for Or<P1, P2>
where
    P1: Matcher,
    P2: Matcher,
{
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        self.pattern1.match_node(node, env).or_else(|| self.pattern2.match_node(node, env))
    }
}

pub struct Inside<Outer> {
    outer: Outer,
}

impl<O> Matcher for Inside<O>
where
    O: Matcher,
{
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

pub struct NotInside<Outer> {
    outer: Outer,
}

impl<O> Matcher for NotInside<O>
where
    O: Matcher,
{
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

pub struct Not<P> {
    not: P,
}

impl<P> Matcher for Not<P>
where
    P: Matcher,
{
    fn match_node<'tree>(&self, node: Node<'tree>, env: &mut MetaVarEnv<'tree>) -> Option<Node<'tree>> {
        if self.not.match_node(node, env).is_none() {
            Some(node)
        } else {
            None
        }
    }
}
