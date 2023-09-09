use crate::RuleConfig;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Doc, Matcher, NodeMatch};

use bit_set::BitSet;
use std::collections::HashMap;

pub struct CombinedScan<'r, L: Language> {
  rules: Vec<&'r RuleConfig<L>>,
  kind_rule_mapping: Vec<Vec<usize>>,
}

impl<'r, L: Language> CombinedScan<'r, L> {
  pub fn new(mut rules: Vec<&'r RuleConfig<L>>) -> Self {
    // process fixable rule first, the order by id
    // note, mapping.push will invert order so we sort fixable order in reverse
    rules.sort_unstable_by_key(|r| (r.fixer.is_some(), &r.id));
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

  pub fn find<D>(&self, root: &AstGrep<D>) -> BitSet
  where
    D: Doc<Lang = L>,
  {
    let mut hit = BitSet::new();
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        if hit.contains(idx) {
          continue;
        }
        let rule = &self.rules[idx];
        if rule.matcher.match_node(node.clone()).is_some() {
          hit.insert(idx);
        }
      }
    }
    hit
  }

  pub fn scan<'a, D>(
    &self,
    root: &'a AstGrep<D>,
    hit: BitSet,
    exclude_fix: bool,
  ) -> HashMap<usize, Vec<NodeMatch<'a, D>>>
  where
    D: Doc<Lang = L>,
  {
    let mut results = HashMap::new();
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        if !hit.contains(idx) {
          continue;
        }
        let rule = &self.rules[idx];
        if exclude_fix && rule.fixer.is_some() {
          continue;
        }
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        let matches = results.entry(idx).or_insert_with(Vec::new);
        matches.push(ret);
      }
    }
    results
  }

  // only visit fixable rules for now
  // NOTE:it may be changed in future
  pub fn diffs<'a, D>(&self, root: &'a AstGrep<D>, hit: BitSet) -> Vec<(NodeMatch<'a, D>, usize)>
  where
    D: Doc<Lang = L>,
  {
    let mut results = vec![];
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        let rule = &self.rules[idx];
        if !hit.contains(idx) || rule.fix.is_none() {
          continue;
        }
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        results.push((ret, idx));
      }
    }
    results
  }

  pub fn get_rule(&self, idx: usize) -> &RuleConfig<L> {
    self.rules[idx]
  }
}
