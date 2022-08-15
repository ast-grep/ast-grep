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
    Inside(Box<RelationalRule>),
    Has(Box<RelationalRule>),
    Precedes(Box<RelationalRule>),
    Follows(Box<RelationalRule>),
    Pattern(PatternStyle),
    Kind(String),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelationalRule {
    #[serde(flatten)]
    rule: SerializableRule,
    #[serde(default)]
    until: Option<SerializableRule>,
    #[serde(default)]
    immediate: bool,
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
    Inside(Box<Inside<L>>),
    Has(Box<Has<L>>),
    Precedes(Box<Precedes<L>>),
    Follows(Box<Follows<L>>),
    Pattern(Pattern<L>),
    Kind(KindMatcher<L>),
}

impl<L: Language> Matcher<L> for Rule<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        use Rule::*;
        match self {
            All(all) => all.match_node_with_env(node, env),
            Any(any) => any.match_node_with_env(node, env),
            Not(not) => not.match_node_with_env(node, env),
            Inside(parent) => match_and_add_label(&**parent, node, env),
            Has(child) => match_and_add_label(&**child, node, env),
            Precedes(latter) => match_and_add_label(&**latter, node, env),
            Follows(former) => match_and_add_label(&**former, node, env),
            Pattern(pattern) => pattern.match_node_with_env(node, env),
            Kind(kind) => kind.match_node_with_env(node, env),
        }
    }
}
fn match_and_add_label<'tree, L: Language, M: Matcher<L>>(
    inner: &M,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
) -> Option<Node<'tree, L>> {
    let matched = inner.match_node_with_env(node, env)?;
    env.add_label("secondary", matched.clone());
    Some(matched)
}

fn until<L: Language>(pattern: &Option<Rule<L>>) -> impl Fn(&Node<L>) -> bool + '_ {
    move |n| {
        if let Some(m) = pattern {
            m.match_node(n.clone()).is_none()
        } else {
            true
        }
    }
}

pub struct Inside<L: Language> {
    outer: Rule<L>,
    until: Option<Rule<L>>,
    immediate: bool,
    lang: PhantomData<L>,
}
impl<L: Language> Inside<L> {
    fn try_new(relation: RelationalRule, lang: L) -> Result<Inside<L>, SerializeError> {
        let util_node = if let Some(until) = relation.until {
            Some(try_from_serializable(until, lang.clone())?)
        } else {
            None
        };
        Ok(Self {
            outer: try_from_serializable(relation.rule, lang)?,
            until: util_node,
            immediate: relation.immediate,
            lang: PhantomData,
        })
    }
}

impl<L: Language> Matcher<L> for Inside<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        if self.immediate {
            self.outer.match_node_with_env(node.parent()?, env)
        } else {
            node.ancestors()
                .take_while(until(&self.until))
                .find_map(|n| self.outer.match_node_with_env(n, env))
        }
    }
}
pub struct Has<L: Language> {
    inner: Rule<L>,
    until: Option<Rule<L>>,
    immediate: bool,
    lang: PhantomData<L>,
}
impl<L: Language> Has<L> {
    fn try_new(relation: RelationalRule, lang: L) -> Result<Self, SerializeError> {
        let util_node = if let Some(until) = relation.until {
            Some(try_from_serializable(until, lang.clone())?)
        } else {
            None
        };
        Ok(Self {
            inner: try_from_serializable(relation.rule, lang)?,
            until: util_node,
            immediate: relation.immediate,
            lang: PhantomData,
        })
    }
}
impl<L: Language> Matcher<L> for Has<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        if self.immediate {
            node.children()
                .find_map(|n| self.inner.match_node_with_env(n, env))
        } else {
            node.dfs()
                .skip(1)
                .take_while(until(&self.until))
                .find_map(|n| self.inner.match_node_with_env(n, env))
        }
    }
}

pub struct Precedes<L: Language> {
    inner: Rule<L>,
    until: Option<Rule<L>>,
    immediate: bool,
    lang: PhantomData<L>,
}
impl<L: Language> Precedes<L> {
    fn try_new(relation: RelationalRule, lang: L) -> Result<Self, SerializeError> {
        let util_node = if let Some(until) = relation.until {
            Some(try_from_serializable(until, lang.clone())?)
        } else {
            None
        };
        Ok(Self {
            inner: try_from_serializable(relation.rule, lang)?,
            until: util_node,
            immediate: relation.immediate,
            lang: PhantomData,
        })
    }
}
impl<L: Language> Matcher<L> for Precedes<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        if self.immediate {
            self.inner.match_node_with_env(node.prev()?, env)
        } else {
            node.prev_all()
                .take_while(until(&self.until))
                .find_map(|n| self.inner.match_node_with_env(n, env))
        }
    }
}

pub struct Follows<L: Language> {
    inner: Rule<L>,
    until: Option<Rule<L>>,
    immediate: bool,
    lang: PhantomData<L>,
}
impl<L: Language> Follows<L> {
    fn try_new(relation: RelationalRule, lang: L) -> Result<Self, SerializeError> {
        let util_node = if let Some(until) = relation.until {
            Some(try_from_serializable(until, lang.clone())?)
        } else {
            None
        };
        Ok(Self {
            inner: try_from_serializable(relation.rule, lang)?,
            until: util_node,
            immediate: relation.immediate,
            lang: PhantomData,
        })
    }
}
impl<L: Language> Matcher<L> for Follows<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        if self.immediate {
            self.inner.match_node_with_env(node.next()?, env)
        } else {
            node.next_all()
                .take_while(until(&self.until))
                .find_map(|n| self.inner.match_node_with_env(n, env))
        }
    }
}

#[derive(Debug)]
pub enum SerializeError {
    MissPositiveMatcher,
}

// TODO: implement positive/non positive
pub fn try_from_serializable<L: Language>(
    serialized: SerializableRule,
    lang: L,
) -> Result<Rule<L>, SerializeError> {
    use Rule as R;
    use SerializableRule as S;
    let mapper = |s| try_from_serializable(s, lang.clone());
    let convert_rules = |rules: Vec<SerializableRule>| {
        let mut inner = Vec::with_capacity(rules.len());
        for rule in rules {
            inner.push(try_from_serializable(rule, lang.clone())?);
        }
        Ok(inner)
    };
    let ret = match serialized {
        S::All(all) => R::All(o::All::new(convert_rules(all)?)),
        S::Any(any) => R::Any(o::Any::new(convert_rules(any)?)),
        S::Not(not) => R::Not(Box::new(o::Not::new(mapper(*not)?))),
        S::Inside(inside) => R::Inside(Box::new(Inside::try_new(*inside, lang)?)),
        S::Has(has) => R::Has(Box::new(Has::try_new(*has, lang)?)),
        S::Precedes(precedes) => R::Precedes(Box::new(Precedes::try_new(*precedes, lang)?)),
        S::Follows(follows) => R::Follows(Box::new(Follows::try_new(*follows, lang)?)),
        S::Kind(kind) => R::Kind(KindMatcher::new(&kind, lang)),
        S::Pattern(PatternStyle::Str(pattern)) => R::Pattern(Pattern::new(&pattern, lang)),
        S::Pattern(PatternStyle::Contextual { context, selector }) => {
            R::Pattern(Pattern::contextual(&context, &selector, lang))
        }
    };
    Ok(ret)
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

    #[test]
    fn test_relational() {
        let src = r"
inside:
    pattern: class A {}
    immediate: true
    until:
        pattern: function() {}
";
        let rule: SerializableRule = from_str(src).expect("cannot parse rule");
        match rule {
            SerializableRule::Inside(rule) => assert!(rule.immediate),
            _ => unreachable!(),
        }
    }
}
