use serde::{Deserialize, Serialize};

use ast_grep_core::language::Language;
use ast_grep_core::{KindMatcher, MetaVarMatcher, Pattern};
use regex::Regex;

#[derive(Serialize, Deserialize, Clone)]
pub enum SerializableMetaVar {
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
    meta_var: SerializableMetaVar,
    lang: L,
) -> Result<MetaVarMatcher<L>, SerializeError> {
    use SerializableMetaVar as S;
    match meta_var {
        S::Regex(s) => match Regex::new(&s) {
            Ok(r) => Ok(MetaVarMatcher::Regex(r)),
            Err(e) => Err(SerializeError::InvalidRegex(e)),
        },
        S::Pattern(p) => Ok(MetaVarMatcher::Pattern(Pattern::new(&p, lang))),
        S::Kind(p) => Ok(MetaVarMatcher::Kind(KindMatcher::new(&p, lang))),
    }
}
