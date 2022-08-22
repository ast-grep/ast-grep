use serde::{Deserialize, Serialize};

use crate::rule::Rule;
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::{KindMatcher, Matcher, MetaVarMatcher, Node, Pattern};
use regex::Regex;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SerializableMetaVarMatcher {
    /// A regex to filter metavar based on its textual content.
    Regex(String),
    /// A pattern to filter matched metavar based on its AST tree shape.
    Pattern(String),
    /// A kind_id to filter matched metavar based on its ts-node kind
    Kind(String),
}

#[derive(Debug)]
pub enum SerializeError {
    InvalidRegex(regex::Error),
    // InvalidPattern,
}

pub fn try_from_serializable<L: Language>(
    meta_var: SerializableMetaVarMatcher,
    lang: L,
) -> Result<MetaVarMatcher<L>, SerializeError> {
    use SerializableMetaVarMatcher as S;
    match meta_var {
        S::Regex(s) => match Regex::new(&s) {
            Ok(r) => Ok(MetaVarMatcher::Regex(r)),
            Err(e) => Err(SerializeError::InvalidRegex(e)),
        },
        S::Pattern(p) => Ok(MetaVarMatcher::Pattern(Pattern::new(&p, lang))),
        S::Kind(p) => Ok(MetaVarMatcher::Kind(KindMatcher::new(&p, lang))),
    }
}

pub fn try_deserialize_matchers<L: Language>(
    meta_vars: HashMap<String, SerializableMetaVarMatcher>,
    lang: L,
) -> Result<MetaVarMatchers<L>, SerializeError> {
    let mut map = MetaVarMatchers::new();
    for (key, matcher) in meta_vars {
        map.insert(key, try_from_serializable(matcher, lang.clone())?);
    }
    Ok(map)
}

pub struct RuleWithConstraint<L: Language> {
    pub rule: Rule<L>,
    pub matchers: MetaVarMatchers<L>,
}

impl<L: Language> Matcher<L> for RuleWithConstraint<L> {
    fn match_node_with_env<'tree>(
        &self,
        node: Node<'tree, L>,
        env: &mut MetaVarEnv<'tree, L>,
    ) -> Option<Node<'tree, L>> {
        self.rule.match_node_with_env(node, env)
    }

    fn get_meta_var_env<'tree>(&self) -> MetaVarEnv<'tree, L> {
        MetaVarEnv::from_matchers(self.matchers.clone())
    }
}
