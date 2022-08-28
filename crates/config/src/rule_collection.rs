use crate::RuleConfig;
use ast_grep_core::language::Language;

use std::collections::HashMap;

pub struct RuleBucket<L: Language> {
    rules: Vec<RuleConfig<L>>,
}

impl<L: Language> RuleBucket<L> {
    pub fn add(&mut self, rule: RuleConfig<L>) {
        self.rules.push(rule);
    }
}

impl<L: Language> Default for RuleBucket<L> {
    fn default() -> Self {
        Self { rules: vec![] }
    }
}

/// A collection of rules to run one round of scanning.
/// Rules will be grouped together based on their language, path globbing and pattern rule.
pub struct RuleCollection<L: Language> {
    // TODO: use tinyvec
    pub tenured: HashMap<String, RuleBucket<L>>,
    pub contingent: Vec<RuleConfig<L>>,
}

impl<L: Language + ToString> RuleCollection<L> {
    pub fn new(configs: Vec<RuleConfig<L>>) -> Self {
        let mut tenured = HashMap::<String, RuleBucket<L>>::new();
        let mut contingent = vec![];
        for config in configs {
            if config.files.is_none() && config.ignores.is_none() {
                let lang = config.language.to_string();
                tenured.entry(lang).or_default().add(config);
            } else {
                contingent.push(config);
            }
        }
        Self {
            tenured,
            contingent,
        }
    }

    // TODO: get rules without allocation
    pub fn get_rules_for_lang(&self, lang: &L) -> Vec<&RuleConfig<L>> {
        let lang = lang.to_string();
        if let Some(bucket) = self.tenured.get(&lang) {
            bucket.rules.iter().collect()
        } else {
            vec![]
        }
    }
}
