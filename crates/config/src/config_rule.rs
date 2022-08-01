use ast_grep_core::{Matcher, Node, KindMatcher, Pattern};
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::rule as r;
use ast_grep_core::language::Language;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SerializableRule {
    All(Vec<SerializableRule>),
    Any(Vec<SerializableRule>),
    Not(Box<SerializableRule>),
    Inside(Box<SerializableRule>),
    Has(Box<SerializableRule>),
    Pattern(String),
    Kind(String),
}

pub enum DynamicRule<L: Language> {
    All(r::All<L, DynamicRule<L>>),
    Any(r::Any<L, DynamicRule<L>>),
    Not(Box<r::Not<L, DynamicRule<L>>>),
    Inside(Box<r::Inside<L, DynamicRule<L>>>),
    Has(Box<r::Has<L, DynamicRule<L>>>),
    Pattern(Pattern<L>),
    Kind(KindMatcher<L>),
}

impl<L: Language> Matcher<L> for DynamicRule<L> {
    fn match_node_with_env<'tree>(&self, node: Node<'tree, L>, env: &mut MetaVarEnv<'tree, L>) -> Option<ast_grep_core::Node<'tree, L>> {
        use DynamicRule::*;
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

enum SerializeError {
    MissPositiveMatcher,
}

// TODO: implement positive/non positive
pub fn from_serializable<L: Language>(serialized: SerializableRule, lang: L) -> DynamicRule<L> {
    use SerializableRule as S;
    use DynamicRule as D;
    let mapper = |s| from_serializable(s, lang);
    match serialized {
        S::All(all) => D::All(r::All::new(all.into_iter().map(mapper))),
        S::Any(any) => D::Any(r::Any::new(any.into_iter().map(mapper))),
        S::Not(not) => D::Not(Box::new(r::Not::new(mapper(*not)))),
        S::Inside(inside) => D::Inside(Box::new(r::Inside::new(mapper(*inside)))),
        S::Has(has) => D::Has(Box::new(r::Has::new(mapper(*has)))),
        S::Pattern(pattern) => D::Pattern(Pattern::new(&pattern, lang)),
        S::Kind(kind) => D::Kind(KindMatcher::new(&kind, lang)),
    }
}
