use crate::matcher::{match_node_non_recursive};
use crate::{meta_var::MetaVarEnv, Node, Root};
use std::collections::VecDeque;

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
    pub fn find_node<'tree>(&self, node: Node<'tree>) -> Option<(Node<'tree>, MetaVarEnv<'tree>)> {
        match &self.pattern_kind {
            PatternKind::NodePattern(ref n) => {
                let root = n.root();
                find_node(root, node)
            }
            PatternKind::KindPattern(k) => find_kind(k, node),
        }
    }

    pub fn find_all_nodes<'tree>(&self, node: Node<'tree>) -> Vec<Node<'tree>> {
        match &self.pattern_kind {
            PatternKind::NodePattern(ref n) => {
                let root = n.root();
                find_node_all(root, node)
            }
            PatternKind::KindPattern(k) => find_kind_iter(k, node).collect(),
        }
    }

    pub fn match_one_node<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        let mut env = MetaVarEnv::new();
        match &self.pattern_kind {
            PatternKind::NodePattern(goal) => match_node_non_recursive(&goal.root(), node, &mut env),
            PatternKind::KindPattern(kind) => if &node.kind() == kind { Some(node) } else { None },
        }
    }
}

impl<'a> From<&'a str> for Pattern {
    fn from(src: &'a str) -> Self {
        Self::new(src)
    }
}

fn find_kind<'tree>(
    kind: &'static str,
    candidate: Node<'tree>,
) -> Option<(Node<'tree>, MetaVarEnv<'tree>)> {
    let mut env = MetaVarEnv::new();
    let node = find_single_kind(kind, candidate, &mut env)?;
    Some((node, env))
}

fn find_node<'goal, 'tree>(
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
    let node = find_node_recursive(&goal, candidate, &mut env)?;
    Some((node, env))
}

fn find_node_all<'goal, 'tree>(goal: Node<'goal>, candidate: Node<'tree>) -> Vec<Node<'tree>> {
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
    find_nodes_iter(&goal, candidate).collect()
}

pub(super) fn find_node_recursive<'goal, 'tree>(
    goal: &Node<'goal>,
    candidate: Node<'tree>,
    env: &mut MetaVarEnv<'tree>,
) -> Option<Node<'tree>> {
    match_node_non_recursive(goal, candidate, env).or_else(|| {
        candidate
            .children()
            .find_map(|sub_cand| find_node_recursive(goal, sub_cand, env))
    })
}

fn find_nodes_iter<'goal, 'tree: 'goal>(
    goal: &'goal Node<'goal>,
    candidate: Node<'tree>,
) -> impl Iterator<Item = Node<'tree>> + 'goal {
    let mut queue = VecDeque::new();
    queue.push_back(candidate);
    std::iter::from_fn(move || loop {
        let cand = queue.pop_front()?;
        queue.extend(cand.children());
        let mut env = MetaVarEnv::new();
        let n = match_node_non_recursive(goal, cand, &mut env);
        if n.is_some() {
            return n;
        }
    })
}

fn find_single_kind<'tree>(
    goal_kind: &str,
    candidate: Node<'tree>,
    env: &mut MetaVarEnv<'tree>,
) -> Option<Node<'tree>> {
    if candidate.kind() == goal_kind {
        // TODO: update env
        // env.insert(meta_var.0.to_owned(), candidate);
        return Some(candidate);
    }
    candidate
        .children()
        .find_map(|sub| find_single_kind(goal_kind, sub, env))
}

fn find_kind_iter<'goal, 'tree: 'goal>(
    goal_kind: &'goal str,
    candidate: Node<'tree>,
) -> impl Iterator<Item = Node<'tree>> + 'goal {
    let mut queue = VecDeque::new();
    queue.push_back(candidate);
    std::iter::from_fn(move || loop {
        let cand = queue.pop_front()?;
        queue.extend(cand.children());
        if cand.kind() == goal_kind {
            return Some(cand);
        }
    })
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
            find_node(goal, cand).is_some(),
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
            find_node(goal, cand).is_none(),
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
        let (_, env) = find_node(goal, cand).unwrap();
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
            find_kind(kind, cand).is_some(),
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
            find_kind(kind, cand).is_none(),
            "goal: {}, candidate: {}",
            kind,
            cand.inner.to_sexp(),
        );
    }
}
