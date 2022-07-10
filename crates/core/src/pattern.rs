use crate::matcher::match_node_non_recursive;
use crate::rule::{Matcher, PositiveMatcher};
use crate::{meta_var::MetaVarEnv, Node, Root};
use crate::language::TSLanguage;

pub enum PatternKind {
    NodePattern(Root),
    KindPattern(&'static str),
}

pub struct Pattern {
    pattern_kind: PatternKind,
}

impl Pattern {
    pub fn new(src: &str, ts_lang: TSLanguage) -> Self {
        let node = Root::new(src, ts_lang);
        let goal = node.root();
        if goal.inner.child_count() != 1 {
            todo!("multi-children pattern is not supported yet.")
        }
        let pattern_kind = PatternKind::NodePattern(node);
        Self { pattern_kind }
    }
    pub fn of_kind(kind: &'static str) -> Self {
        Self {
            pattern_kind: PatternKind::KindPattern(kind),
        }
    }
}

impl Matcher for Pattern {
    fn match_node<'tree>(
        &self,
        node: Node<'tree>,
        env: &mut MetaVarEnv<'tree>,
    ) -> Option<Node<'tree>> {
        match &self.pattern_kind {
            PatternKind::NodePattern(goal) => match_node_non_recursive(&matcher(&goal), node, env),
            PatternKind::KindPattern(kind) => {
                if &node.kind() == kind {
                    Some(node)
                } else {
                    None
                }
            }
        }
    }
}

// TODO: extract out matcher in recursion
fn matcher(goal: &Root) -> Node {
    let mut node = goal.root().inner;
    let source = goal.root().source;
    while node.child_count() == 1 {
        node = node.child(0).unwrap();
    }
    let goal = Node {
        inner: node,
        source,
    };
    goal
}

impl PositiveMatcher for Pattern {}

// impl<S: AsRef<str>> From<S> for Pattern {
//     fn from(src: S) -> Self {
//         Self::new(src.as_ref())
//     }
// }

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;
    use crate::language::{Tsx, Language};

    fn pattern_node(s: &str) -> Root {
        let pattern = Pattern::new(s, Tsx::get_ts_language());
        match pattern.pattern_kind {
            PatternKind::NodePattern(n) => n,
            _ => panic!("kind pattern is not supported"),
        }
    }

    fn test_match(s1: &str, s2: &str) {
        let goal = pattern_node(s1);
        let pattern = Pattern {
            pattern_kind: PatternKind::NodePattern(goal),
        };
        let goal = pattern_node(s1);
        let cand = pattern_node(s2);
        let cand = cand.root();
        let mut env = MetaVarEnv::new();
        assert!(
            pattern.find_node(cand, &mut env).is_some(),
            "goal: {}, candidate: {}",
            goal.root().inner.to_sexp(),
            cand.inner.to_sexp(),
        );
    }
    fn test_non_match(s1: &str, s2: &str) {
        let goal = pattern_node(s1);
        let pattern = Pattern {
            pattern_kind: PatternKind::NodePattern(goal),
        };
        let goal = pattern_node(s1);
        let cand = pattern_node(s2);
        let cand = cand.root();
        let mut env = MetaVarEnv::new();
        assert!(
            pattern.find_node(cand, &mut env).is_none(),
            "goal: {}, candidate: {}",
            goal.root().inner.to_sexp(),
            cand.inner.to_sexp(),
        );
    }

    #[test]
    fn test_meta_variable() {
        test_match("const a = $VALUE", "const a = 123");
        test_match("const $VARIABLE = $VALUE", "const a = 123");
        test_match("const $VARIABLE = $VALUE", "const a = 123");
    }

    fn match_env(goal_str: &str, cand: &str) -> HashMap<String, String> {
        let goal = pattern_node(goal_str);
        let pattern = Pattern {
            pattern_kind: PatternKind::NodePattern(goal),
        };
        let cand = pattern_node(cand);
        let cand = cand.root();
        let mut env = MetaVarEnv::new();
        pattern.find_node(cand, &mut env).unwrap();
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
        let pattern = Pattern::of_kind(kind);
        let mut env = MetaVarEnv::new();
        assert!(
            pattern.find_node(cand, &mut env).is_some(),
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
        let pattern = Pattern::of_kind(kind);
        let mut env = MetaVarEnv::new();
        assert!(
            pattern.find_node(cand, &mut env).is_none(),
            "goal: {}, candidate: {}",
            kind,
            cand.inner.to_sexp(),
        );
    }

    #[test]
    fn test_return() {
        test_match("$A($B)", "return test(123)");
    }
}
