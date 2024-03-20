use crate::RuleConfig;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Doc, Matcher, Node, NodeMatch};

use bit_set::BitSet;
use std::collections::{HashMap, HashSet};

/// A struct to group all rules according to their potential kinds.
/// This can greatly reduce traversal times and skip unmatchable rules.
/// Rules are referenced by their index in the rules vector.
pub struct CombinedScan<'r, L: Language> {
  rules: Vec<&'r RuleConfig<L>>,
  /// a vec of vec, mapping from kind to a list of rule index
  kind_rule_mapping: Vec<Vec<usize>>,
}

impl<'r, L: Language> CombinedScan<'r, L> {
  pub fn new(mut rules: Vec<&'r RuleConfig<L>>) -> Self {
    // process fixable rule first, the order by id
    // note, mapping.push will invert order so we sort fixable order in reverse
    rules.sort_unstable_by_key(|r| (r.fix.is_some(), &r.id));
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
      let mut suppression = NodeSuppression::new(node.clone());
      for &idx in rule_idx {
        if !hit.contains(idx) {
          continue;
        }
        let rule = &self.rules[idx];
        if exclude_fix && rule.fix.is_some() {
          continue;
        }
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        if suppression.check_ignore_all() {
          break;
        }
        if suppression.contains_rule(&rule.id) {
          break;
        }
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
      let mut suppression = NodeSuppression::new(node.clone());
      for &idx in rule_idx {
        let rule = &self.rules[idx];
        if suppression.contains_rule(&rule.id) {
          continue;
        }
        if !hit.contains(idx) || rule.fix.is_none() {
          continue;
        }
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        if suppression.check_ignore_all() {
          break;
        }
        if !suppression.contains_rule(&rule.id) {
          results.push((ret, idx));
        }
      }
    }
    results
  }

  pub fn get_rule(&self, idx: usize) -> &RuleConfig<L> {
    self.rules[idx]
  }
}

enum NodeSuppression<'r, D: Doc> {
  Unchecked(Node<'r, D>),
  NoSuppression,
  AllSuppressed,
  Specific(HashSet<String>),
}

impl<'r, D: Doc> NodeSuppression<'r, D> {
  fn new(node: Node<'r, D>) -> Self {
    Self::Unchecked(node)
  }
  /// returns if a rule should be suppressed
  /// the return value is always false before check_ignore_all is called
  /// that is, we never ignore error before looking at its surrounding
  /// so this method needs to be called twice
  fn contains_rule(&self, id: &str) -> bool {
    use NodeSuppression::*;
    match self {
      Unchecked(_) => false,
      NoSuppression => false,
      AllSuppressed => panic!("AllSuppression should never be called"),
      Specific(set) => set.contains(id),
    }
  }
  /// this method will lazily check suppression
  /// contains_rule will only return truth after this
  fn check_ignore_all(&mut self) -> bool {
    use NodeSuppression::*;
    *self = match self {
      Unchecked(n) => suppressed(n),
      AllSuppressed => panic!("impossible"),
      _ => return false,
    };
    matches!(self, AllSuppressed)
  }
}

// check if there is no ast-grep-ignore
fn suppressed<'r, D: Doc>(node: &Node<'r, D>) -> NodeSuppression<'r, D> {
  let mut node = node.clone();
  use NodeSuppression::*;
  loop {
    let Some(prev) = node.prev() else {
      let Some(n) = node.parent() else {
        return NoSuppression;
      };
      node = n;
      continue;
    };
    if prev.start_pos().0 == node.start_pos().0 {
      let Some(n) = node.parent() else {
        return NoSuppression;
      };
      node = n;
    } else if prev.start_pos().0 + 1 == node.start_pos().0 {
      return parse_suppression(&prev.text());
    } else {
      return NoSuppression;
    }
  }
}

fn parse_suppression<'r, D: Doc>(text: &str) -> NodeSuppression<'r, D> {
  let Some((_, after)) = text.trim().split_once("ast-grep-ignore") else {
    return NodeSuppression::NoSuppression;
  };
  let after = after.trim();
  if after.is_empty() {
    return NodeSuppression::AllSuppressed;
  }
  let Some((_, rules)) = after.split_once(':') else {
    return NodeSuppression::AllSuppressed;
  };
  let set = rules.split(',').map(|r| r.trim().to_string()).collect();
  NodeSuppression::Specific(set)
}
