use crate::RuleConfig;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Matcher, NodeMatch, StrDoc};

use std::collections::HashMap;

pub struct CombinedScan<'r, L: Language> {
  rules: Vec<&'r RuleConfig<L>>,
  kind_rule_mapping: Vec<Vec<usize>>,
}

impl<'r, L: Language> CombinedScan<'r, L> {
  pub fn new(rules: Vec<&'r RuleConfig<L>>) -> Self {
    let mut mapping = Vec::new();
    for (idx, rule) in rules.iter().enumerate() {
      for kind in &rule
        .matcher
        .potential_kinds()
        .unwrap_or_else(|| panic!("rule `{}` must have kind", &rule.id))
      {
        // NOTE: common languages usually have about several hundred kinds
        // from 200+ ~ 500+, it is okay to waste about 500 * 24 Byte vec size = 12kB
        // see https://github.com/Wilfred/difftastic/tree/master/vendored_parsers
        while mapping.len() <= kind {
          mapping.push(vec![]);
        }
        mapping[kind].push(idx);
      }
    }
    Self {
      rules,
      kind_rule_mapping: mapping,
    }
  }

  pub fn find(&self, root: &AstGrep<StrDoc<L>>) -> bool {
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        let rule = &self.rules[idx];
        if rule.matcher.match_node(node.clone()).is_some() {
          return true;
        }
      }
    }
    false
  }

  pub fn scan<'a>(
    &self,
    root: &'a AstGrep<StrDoc<L>>,
  ) -> HashMap<usize, Vec<NodeMatch<'a, StrDoc<L>>>> {
    let mut results = HashMap::new();
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        let rule = &self.rules[idx];
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        let matches = results.entry(idx).or_insert_with(Vec::new);
        matches.push(ret);
      }
    }
    results
  }

  pub fn get_rule(&self, idx: usize) -> &RuleConfig<L> {
    self.rules[idx]
  }
}
