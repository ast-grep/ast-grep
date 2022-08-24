use crate::RuleConfig;
use ast_grep_core::language::Language;
use std::collections::HashMap;

pub struct RuleBucket<L: Language> {
    rules: Vec<Vec<RuleConfig<L>>>,
}

/// A collection of rules to run one round of scanning.
/// Rules will be grouped together based on their language, path globbing and pattern rule.
pub struct RuleConfigCollection<L: Language> {
    // TODO: use tinyvec
    pub tenured: HashMap<String, RuleBucket<L>>,
    pub contingent: Vec<RuleConfig<L>>,
}

impl<L: Language> RuleConfigCollection<L> {
    pub fn new(configs: Vec<RuleConfig<L>>) -> Self {
        todo!()
    }
}
