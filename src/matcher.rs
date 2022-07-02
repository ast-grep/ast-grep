use crate::meta_var::{MetaVarEnv, extract_meta_var, MetaVariable};
use crate::Node;
use std::collections::VecDeque;

pub fn match_single_kind<'tree>(
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
        .find_map(|sub| match_single_kind(goal_kind, sub, env))
}

pub fn match_kind_iter<'goal, 'tree: 'goal>(
    goal_kind: &'goal str,
    candidate: Node<'tree>,
) -> impl Iterator<Item=Node<'tree>> + 'goal {
    let mut stack = vec![candidate];
    std::iter::from_fn(move || loop {
        let cand = stack.pop()?;
        stack.extend(cand.children());
        if cand.kind() == goal_kind {
            return Some(cand);
        }
    })
}


fn match_leaf_meta_var<'goal, 'tree>(
    goal: &Node<'goal>,
    candidate: Node<'tree>,
    env: &mut MetaVarEnv<'tree>,
) -> Option<Node<'tree>> {
    let extracted = extract_var_from_node(goal)?;
    use MetaVariable as MV;
    match extracted {
        MV::Named(name) => {
            env.insert(name, candidate)?;
            Some(candidate)
        }
        MV::Anonymous => Some(candidate),
        // Ellipsis will be matched in parent level
        MV::Ellipsis => Some(candidate),
        MV::NamedEllipsis(name) => {
            env.insert(name, candidate)?;
            Some(candidate)
        }
    }
}

fn is_ellipsis(node: &Node) -> bool {
    matches!(
        extract_var_from_node(node),
        Some(MetaVariable::Ellipsis | MetaVariable::NamedEllipsis(_))
    )
}

fn match_node_non_recursive<'goal, 'tree>(
    goal: &Node<'goal>,
    candidate: Node<'tree>,
    env: &mut MetaVarEnv<'tree>,
) -> Option<Node<'tree>> {
    let is_leaf = goal.is_leaf();
    if is_leaf {
        if let Some(matched) = match_leaf_meta_var(goal, candidate, env) {
            return Some(matched);
        }
    }
    if goal.kind_id() != candidate.kind_id() {
        return None;
    }
    if is_leaf {
        if extract_var_from_node(goal).is_some() {
            return None;
        }
        return if goal.text() == candidate.text() {
            Some(candidate)
        } else {
            None
        };
    }
    let mut goal_children = goal.children().peekable();
    let mut cand_children = candidate.children().peekable();
    cand_children.peek()?;
    loop {
        let curr_node = goal_children.peek().unwrap();
        if is_ellipsis(curr_node) {
            // goal has all matched
            goal_children.next();
            if goal_children.peek().is_none() {
                // TODO: update env
                return Some(candidate);
            }
            while !goal_children.peek().unwrap().inner.is_named() {
                goal_children.next();
                if goal_children.peek().is_none() {
                    // TODO: update env
                    return Some(candidate);
                }
            }
            // if next node is a Ellipsis, consume one candidate node
            if is_ellipsis(goal_children.peek().unwrap()) {
                cand_children.next();
                cand_children.peek()?;
                // TODO: update env
                continue;
            }
            loop {
                if match_node_non_recursive(
                    goal_children.peek().unwrap(),
                    *cand_children.peek().unwrap(),
                    env,
                )
                .is_some()
                {
                    // found match non Ellipsis,
                    break;
                }
                cand_children.next();
                cand_children.peek()?;
            }
        }
        match_node_non_recursive(
            goal_children.peek().unwrap(),
            *cand_children.peek().unwrap(),
            env,
        )?;
        goal_children.next();
        if goal_children.peek().is_none() {
            // all goal found, return
            return Some(candidate);
        }
        cand_children.next();
        cand_children.peek()?;
    }
}

pub fn does_node_match_exactly(goal: &Node, candidate: Node) -> bool {
    if goal.kind_id() != candidate.kind_id() {
        return false;
    }
    if goal.is_leaf() {
        return goal.text() == candidate.text();
    }
    let goal_children = goal.children();
    let cand_children = candidate.children();
    if goal_children.len() != cand_children.len() {
        return false;
    }
    goal_children
        .zip(cand_children)
        .all(|(g, c)| does_node_match_exactly(&g, c))
}

fn extract_var_from_node(goal: &Node) -> Option<MetaVariable> {
    let key = goal.text();
    extract_meta_var(key)
}

pub fn match_node_recursive<'goal, 'tree>(
    goal: &Node<'goal>,
    candidate: Node<'tree>,
    env: &mut MetaVarEnv<'tree>,
) -> Option<Node<'tree>> {
    match_node_non_recursive(goal, candidate, env).or_else(|| {
        candidate
            .children()
            .find_map(|sub_cand| match_node_recursive(goal, sub_cand, env))
    })
}

pub fn match_nodes_iter<'goal, 'tree: 'goal>(
    goal: &'goal Node<'goal>,
    candidate: Node<'tree>,
) -> impl Iterator<Item=Node<'tree>> + 'goal {
    let mut stack = VecDeque::new();
    stack.push_back(candidate);
    std::iter::from_fn(move || loop {
        let cand = stack.pop_front()?;
        stack.extend(cand.children());
        let mut env = MetaVarEnv::new();
        let n = match_node_non_recursive(goal, cand, &mut env);
        if n.is_some() {
            return n;
        }
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ts_parser::parse;
    use std::collections::HashMap;

    fn test_match(s1: &str, s2: &str) -> HashMap<String, String> {
        let goal = parse(s1);
        let goal = Node {
            inner: goal.root_node().child(0).unwrap(),
            source: s1,
        };
        let cand = parse(s2);
        let cand = Node {
            inner: cand.root_node(),
            source: s2,
        };
        let mut env = MetaVarEnv::new();
        let ret = match_node_recursive(&goal, cand, &mut env);
        assert!(
            ret.is_some(),
            "goal: {}, candidate: {}",
            goal.inner.to_sexp(),
            cand.inner.to_sexp(),
        );
        HashMap::from(env)
    }

    fn test_non_match(s1: &str, s2: &str) {
        let goal = parse(s1);
        let goal = Node {
            inner: goal.root_node().child(0).unwrap(),
            source: s1,
        };
        let cand = parse(s2);
        let cand = Node {
            inner: cand.root_node(),
            source: s2,
        };
        let mut env = MetaVarEnv::new();
        let ret = match_node_recursive(&goal, cand, &mut env);
        assert!(ret.is_none());
    }

    #[test]
    fn test_simple_match() {
        test_match("const a = 123", "const a=123");
        test_non_match("const a = 123", "var a = 123");
    }

    #[test]
    fn test_nested_match() {
        test_match("const a = 123", "function() {const a= 123;}");
        test_match("const a = 123", "class A { constructor() {const a= 123;}}");
        test_match(
            "const a = 123",
            "for (let a of []) while (true) { const a = 123;}",
        );
    }

    #[test]
    fn test_should_exactly_match() {
        test_match(
            "function foo() { let a = 123; }",
            "function foo() { let a = 123; }",
        );
        test_non_match(
            "function foo() { let a = 123; }",
            "function bar() { let a = 123; }",
        );
    }
    #[test]
    fn test_match_inner() {
        test_match(
            "function bar() { let a = 123; }",
            "function foo() { function bar() {let a = 123; }}",
        );
        test_non_match(
            "function foo() { let a = 123; }",
            "function foo() { function bar() {let a = 123; }}",
        );
    }

    #[test]
    fn test_single_ellipsis() {
        test_match("foo($$$)", "foo(a, b, c)");
        test_match("foo($$$)", "foo()");
    }
    #[test]
    fn test_named_ellipsis() {
        test_match("foo($$$A, c)", "foo(a, b, c)");
        test_match("foo($$$A, b, c)", "foo(a, b, c)");
        test_match("foo($$$A, a, b, c)", "foo(a, b, c)");
        test_non_match("foo($$$A, a, b, c)", "foo(b, c)");
    }

    #[test]
    fn test_leading_ellipsis() {
        test_match("foo($$$, c)", "foo(a, b, c)");
        test_match("foo($$$, b, c)", "foo(a, b, c)");
        test_match("foo($$$, a, b, c)", "foo(a, b, c)");
        test_non_match("foo($$$, a, b, c)", "foo(b, c)");
    }
    #[test]
    fn test_trailing_ellipsis() {
        test_match("foo(a, $$$)", "foo(a, b, c)");
        test_match("foo(a, b, $$$)", "foo(a, b, c)");
        // test_match("foo(a, b, c, $$$)", "foo(a, b, c)");
        test_non_match("foo(a, b, c, $$$)", "foo(b, c)");
    }

    #[test]
    fn test_meta_var_multiple_occurrence() {
        test_match("$A($$$)", "test(123)");
        test_match("$A($B)", "test(123)");
        test_non_match("$A($A)", "test(aaa)");
        test_non_match("$A($A)", "test(123)");
        test_non_match("$A($A, $A)", "test(123, 456)");
        test_match("$A($A)", "test(test)");
    }

    #[test]
    fn test_return() {
        // test_match("$A($B)", "return test(123)");
    }
}
