use crate::meta_var::MetaVarEnv;
use crate::Node;

pub trait Matcher {
    fn match_node<'tree>(&self, _node: Node<'tree>) -> Option<Node<'tree>> {
        None
    }
}

pub struct Rule<'tree, M: Matcher> {
    env: MetaVarEnv<'tree>,
    matcher: M,
}

impl<'tree, M: Matcher> Rule<'tree, M> {
    pub fn match_node(&mut self, node: Node<'tree>) -> Option<Node<'tree>> {
        self.matcher.match_node(node)
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
    fn match_node<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        todo!()
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
    fn match_node<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        todo!()
    }
}

pub struct Inside<Outer, Inner> {
    outer: Outer,
    inner: Inner,
}

impl<O, I> Matcher for Inside<O, I>
where
    O: Matcher,
    I: Matcher,
{
    fn match_node<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        todo!()
    }
}

pub struct NotInside<Outer, Inner> {
    outer: Outer,
    inner: Inner,
}

impl<O, I> Matcher for NotInside<O, I>
where
    O: Matcher,
    I: Matcher,
{
    fn match_node<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        todo!()
    }
}

pub struct Not<P> {
    not: P,
}

impl<P> Matcher for Not<P>
where
    P: Matcher,
{
    fn match_node<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        todo!()
    }
}
