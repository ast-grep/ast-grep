use crate::language::Language;
use crate::match_tree::match_node_non_recursive;
use crate::matcher::{KindMatcher, Matcher, PositiveMatcher};
use crate::{meta_var::MetaVarEnv, Node, Root};

#[derive(Clone)]
pub struct Pattern<L: Language> {
    pub root: Root<L>,
    selector: Option<KindMatcher<L>>,
}

impl<L: Language> Pattern<L> {
    pub fn new(src: &str, lang: L) -> Self {
        let root = Root::new(src, lang);
        let goal = root.root();
        if goal.inner.child_count() != 1 {
            todo!("multi-children pattern is not supported yet.")
        }
        Self {
            root,
            selector: None,
        }
    }

    pub fn contextual(context: &str, selector: &str, lang: L) -> Self {
        let root = Root::new(context, lang.clone());
        let goal = root.root();
        if goal.inner.child_count() != 1 {
            todo!("multi-children pattern is not supported yet.")
        }
        let kind_matcher = KindMatcher::new(selector, lang);
        if goal.find(&kind_matcher).is_none() {
            todo!("use result to indicate failure");
        }
        Self {
            root,
            selector: Some(kind_matcher),
        }
    }

    // TODO: extract out matcher in recursion
    fn matcher(&self) -> Node<L> {
        let root = self.root.root();
        if let Some(kind_matcher) = &self.selector {
            return root
                .find(kind_matcher)
                .expect("contextual match should succeed");
        }
        let mut node = root.inner;
        while node.child_count() == 1 {
            node = node.child(0).unwrap();
        }
        Node {
            inner: node,
            root: &self.root,
        }
    }
}

impl<L: Language> Matcher<L> for Pattern<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        match_node_non_recursive(&self.matcher(), node, env)
    }
}

impl<L: Language> std::fmt::Debug for Pattern<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.matcher().inner.to_sexp())
    }
}

impl<L: Language> PositiveMatcher<L> for Pattern<L> {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::language::Tsx;
    use std::collections::HashMap;

    fn pattern_node(s: &str) -> Root<Tsx> {
        Root::new(s, Tsx)
    }

    fn test_match(s1: &str, s2: &str) {
        let pattern = Pattern::new(s1, Tsx);
        let cand = pattern_node(s2);
        let cand = cand.root();
        assert!(
            pattern.find_node(cand.clone()).is_some(),
            "goal: {}, candidate: {}",
            pattern.root.root().inner.to_sexp(),
            cand.inner.to_sexp(),
        );
    }
    fn test_non_match(s1: &str, s2: &str) {
        let pattern = Pattern::new(s1, Tsx);
        let cand = pattern_node(s2);
        let cand = cand.root();
        assert!(
            pattern.find_node(cand.clone()).is_none(),
            "goal: {}, candidate: {}",
            pattern.root.root().inner.to_sexp(),
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
        let pattern = Pattern::new(goal_str, Tsx);
        let cand = pattern_node(cand);
        let cand = cand.root();
        let mut env = MetaVarEnv::new();
        pattern.find_node_with_env(cand, &mut env).unwrap();
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
    fn test_return() {
        test_match("$A($B)", "return test(123)");
    }

    #[test]
    fn test_contextual_pattern() {
        let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
        let cand = pattern_node("class B { b = 123 }");
        assert!(pattern.find_node(cand.root()).is_some());
        let cand = pattern_node("let b = 123");
        assert!(pattern.find_node(cand.root()).is_none());
    }

    #[test]
    fn test_contextual_match_with_env() {
        let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
        let cand = pattern_node("class B { b = 123 }");
        let mut env = MetaVarEnv::new();
        assert!(pattern.find_node_with_env(cand.root(), &mut env).is_some());
        let env = HashMap::from(env);
        assert_eq!(env["F"], "b");
        assert_eq!(env["I"], "123");
    }

    #[test]
    fn test_contextual_unmatch_with_env() {
        let pattern = Pattern::contextual("class A { $F = $I }", "public_field_definition", Tsx);
        let cand = pattern_node("let b = 123");
        let mut env = MetaVarEnv::new();
        assert!(pattern.find_node_with_env(cand.root(), &mut env).is_none());
        let env = HashMap::from(env);
        assert!(env.is_empty());
    }
}
