use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::ops as o;
use ast_grep_core::{KindMatcher, Matcher, Node, Pattern};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SerializableRule {
    All(Vec<SerializableRule>),
    Any(Vec<SerializableRule>),
    Not(Box<SerializableRule>),
    Inside(Box<SerializableRule>),
    Has(Box<SerializableRule>),
    Pattern(PatternStyle),
    Kind(String),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PatternStyle {
    Str(String),
    Contextual { context: String, selector: String },
}

pub enum Rule<L: Language> {
    All(o::All<L, Rule<L>>),
    Any(o::Any<L, Rule<L>>),
    Not(Box<o::Not<L, Rule<L>>>),
    Inside(Box<Inside<L, Rule<L>>>),
    Has(Box<Has<L, Rule<L>>>),
    Pattern(Pattern<L>),
    Kind(KindMatcher<L>),
}

impl<L: Language> Matcher<L> for Rule<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<ast_grep_core::Node<'tree, L>> {
        use Rule::*;
        match self {
            All(all) => all.match_node_with_env(node, env),
            Any(any) => any.match_node_with_env(node, env),
            Not(not) => not.match_node_with_env(node, env),
            Inside(inside) => inside.match_node_with_env(node, env),
            Has(has) => has.match_node_with_env(node, env),
            Pattern(pattern) => pattern.match_node_with_env(node, env),
            Kind(kind) => kind.match_node_with_env(node, env),
        }
    }
}

pub struct Inside<L: Language, M: Matcher<L>> {
    outer: M,
    lang: PhantomData<L>,
}
impl<L: Language, M: Matcher<L>> Inside<L, M> {
    pub fn new(outer: M) -> Self {
        Self {
            outer,
            lang: PhantomData,
        }
    }
}

impl<L: Language, M: Matcher<L>> Matcher<L> for Inside<L, M> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        node.ancestors()
            .find_map(|n| self.outer.match_node_with_env(n, env))
            .map(|_| node)
    }
}

pub struct Has<L: Language, M: Matcher<L>> {
    inner: M,
    lang: PhantomData<L>,
}
impl<L: Language, M: Matcher<L>> Has<L, M> {
    pub fn new(inner: M) -> Self {
        Self {
            inner,
            lang: PhantomData,
        }
    }
}
impl<L: Language, M: Matcher<L>> Matcher<L> for Has<L, M> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        node.dfs()
            .skip(1)
            .find_map(|n| self.inner.match_node_with_env(n, env))
            .map(|_| node)
    }
}

enum SerializeError {
    MissPositiveMatcher,
}

// TODO: implement positive/non positive
pub fn from_serializable<L: Language>(serialized: SerializableRule, lang: L) -> Rule<L> {
    use Rule as R;
    use SerializableRule as S;
    let mapper = |s| from_serializable(s, lang);
    match serialized {
        S::All(all) => R::All(o::All::new(all.into_iter().map(mapper))),
        S::Any(any) => R::Any(o::Any::new(any.into_iter().map(mapper))),
        S::Not(not) => R::Not(Box::new(o::Not::new(mapper(*not)))),
        S::Inside(inside) => R::Inside(Box::new(Inside::new(mapper(*inside)))),
        S::Has(has) => R::Has(Box::new(Has::new(mapper(*has)))),
        S::Kind(kind) => R::Kind(KindMatcher::new(&kind, lang)),
        S::Pattern(PatternStyle::Str(pattern)) => R::Pattern(Pattern::new(&pattern, lang)),
        S::Pattern(PatternStyle::Contextual { context, selector }) => {
            R::Pattern(Pattern::contextual(&context, &selector, lang))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_yaml::from_str;
    use PatternStyle::*;
    use SerializableRule::*;

    #[test]
    fn test_pattern() {
        let src = r"
pattern: Test
";
        let rule: SerializableRule = from_str(src).expect("cannot parse rule");
        assert!(matches!(rule, Pattern(Str(_))));
        let src = r"
pattern:
    context: class $C { set $B() {} }
    selector: method_definition
";
        let rule: SerializableRule = from_str(src).expect("cannot parse rule");
        assert!(matches!(rule, Pattern(Contextual { .. })));
    }
}
