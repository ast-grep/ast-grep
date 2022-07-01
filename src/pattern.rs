use crate::{Root, Node, meta_var::MetaVarEnv};
use crate::matcher::{match_node_recursive, match_single_kind};

pub enum PatternKind {
    NodePattern(Root),
    KindPattern(&'static str),
}

pub struct Pattern {
    pattern_kind: PatternKind,
}

impl Pattern {
    pub fn new(src: &str) -> Self {
        let node = Root::new(src);
        let pattern_kind = PatternKind::NodePattern(node);
        Self { pattern_kind }
    }
    pub fn of_kind(kind: &'static str) -> Self {
        Self {
            pattern_kind: PatternKind::KindPattern(kind),
        }
    }
    pub fn match_node<'tree>(&self, node: Node<'tree>) -> Option<(Node<'tree>, MetaVarEnv<'tree>)> {
        match &self.pattern_kind {
            PatternKind::NodePattern(ref n) => {
                let root = n.root();
                match_node(root, node)
            }
            PatternKind::KindPattern(k) => match_kind(k, node),
        }
    }

    pub fn gen_replaced(&self, _vars: MetaVarEnv) -> String {
        todo!()
    }
}

impl<'a> From<&'a str> for Pattern {
    fn from(src: &'a str) -> Self {
        Self::new(src)
    }
}

fn match_kind<'tree>(
    kind: &'static str,
    candidate: Node<'tree>,
) -> Option<(Node<'tree>, MetaVarEnv<'tree>)> {
    let mut env = MetaVarEnv::new();
    let node = match_single_kind(kind, candidate, &mut env)?;
    Some((node, env))
}

fn match_node<'goal, 'tree>(
    goal: Node<'goal>,
    candidate: Node<'tree>,
) -> Option<(Node<'tree>, MetaVarEnv<'tree>)> {
    let mut env = MetaVarEnv::new();
    let source = &goal.source;
    let cand = &candidate.source;
    let goal = goal.inner;
    if goal.child_count() != 1 {
        todo!("multi-children pattern is not supported yet.")
    }
    let goal = Node {
        inner: goal.child(0).unwrap(),
        source,
    };
    let candidate = candidate.inner;
    if candidate.next_sibling().is_some() {
        todo!("multi candidate roots are not supported yet.")
    }
    let candidate = Node {
        inner: candidate,
        source: cand,
    };
    let node = match_node_recursive(&goal, candidate, &mut env)?;
    Some((node, env))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    fn pattern_node(s: &str) -> Root {
        let pattern = Pattern::new(s);
        match pattern.pattern_kind {
            PatternKind::NodePattern(n) => n,
            _ => panic!("kind pattern is not supported"),
        }
    }

    fn test_match(s1: &str, s2: &str) {
        let goal = pattern_node(s1);
        let goal = goal.root();
        let cand = pattern_node(s2);
        let cand = cand.root();
        assert!(
            match_node(goal, cand).is_some(),
            "goal: {}, candidate: {}",
            goal.inner.to_sexp(),
            cand.inner.to_sexp(),
        );
    }
    fn test_non_match(s1: &str, s2: &str) {
        let goal = pattern_node(s1);
        let goal = goal.root();
        let cand = pattern_node(s2);
        let cand = cand.root();
        assert!(
            match_node(goal, cand).is_none(),
            "goal: {}, candidate: {}",
            goal.inner.to_sexp(),
            cand.inner.to_sexp(),
        );
    }

    #[test]
    fn test_meta_variable() {
        test_match("const a = $VALUE", "const a = 123");
        test_match("const $VARIABLE = $VALUE", "const a = 123");
        test_match("const $VARIABLE = $VALUE", "const a = 123");
    }

    fn match_env(goal: &str, cand: &str) -> HashMap<String, String> {
        let goal = pattern_node(goal);
        let goal = goal.root();
        let cand = pattern_node(cand);
        let cand = cand.root();
        let (_, env) = match_node(goal, cand).unwrap();
        HashMap::from(env)
    }

    #[test]
    fn test_meta_variable_env() {
        let env = match_env("const a = $VALUE", "const a = 123");
        assert_eq!(env["VALUE"], "123");
    }

    #[test]
    fn test_match_non_atomic() {
        let env = match_env("const a = $VALUE", "const a = 5 + 3");
        assert_eq!(env["VALUE"], "5 + 3");
    }

    #[test]
    fn test_class_assignment() {
        test_match("class $C { $MEMBER = $VAL}", "class A {a = 123}");
        test_non_match("class $C { $MEMBER = $VAL; b = 123; }", "class A {a = 123}");
        // test_match("a = 123", "class A {a = 123}");
        test_non_match("a = 123", "class B {b = 123}");
    }

    #[test]
    fn test_kind_match() {
        let kind = "public_field_definition";
        let cand = pattern_node("class A { a = 123 }");
        let cand = cand.root();
        assert!(
            match_kind(kind, cand).is_some(),
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
        assert!(
            match_kind(kind, cand).is_none(),
            "goal: {}, candidate: {}",
            kind,
            cand.inner.to_sexp(),
        );
    }
}
