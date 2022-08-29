use crate::RuleConfig;
use ast_grep_core::language::Language;

pub struct RuleBucket<L: Language> {
  rules: Vec<RuleConfig<L>>,
  lang: L,
}

impl<L: Language> RuleBucket<L> {
  fn new(lang: L) -> Self {
    Self {
      rules: vec![],
      lang,
    }
  }
  pub fn add(&mut self, rule: RuleConfig<L>) {
    self.rules.push(rule);
  }
}

/// A collection of rules to run one round of scanning.
/// Rules will be grouped together based on their language, path globbing and pattern rule.
pub struct RuleCollection<L: Language + Eq> {
  // use vec since we don't have many languages
  pub tenured: Vec<RuleBucket<L>>,
  pub contingent: Vec<RuleConfig<L>>,
}

impl<L: Language + Eq> RuleCollection<L> {
  pub fn new(configs: Vec<RuleConfig<L>>) -> Self {
    let mut tenured = vec![];
    let mut contingent = vec![];
    for config in configs {
      if config.files.is_none() && config.ignores.is_none() {
        Self::add_tenured_rule(&mut tenured, config);
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
    // TODO: add contingent
    for rule in &self.tenured {
      if &rule.lang == lang {
        return rule.rules.iter().collect();
      }
    }
    vec![]
  }

  fn add_tenured_rule(tenured: &mut Vec<RuleBucket<L>>, rule: RuleConfig<L>) {
    let lang = rule.language.clone();
    for bucket in tenured.iter_mut() {
      if bucket.lang == lang {
        bucket.add(rule);
        return;
      }
    }
    let mut bucket = RuleBucket::new(lang);
    bucket.add(rule);
    tenured.push(bucket);
  }
}
