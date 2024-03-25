use crate::RuleConfig;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Doc, Matcher, Node, NodeMatch};

use bit_set::BitSet;
use std::collections::{HashMap, HashSet};

pub struct ScanResult<'r, D: Doc> {
  pub diffs: Vec<(usize, NodeMatch<'r, D>)>,
  pub matches: HashMap<usize, Vec<NodeMatch<'r, D>>>,
}

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
    separate_fix: bool,
  ) -> ScanResult<'a, D>
  where
    D: Doc<Lang = L>,
  {
    let mut result = ScanResult {
      diffs: vec![],
      matches: HashMap::new(),
    };
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
        if suppression.contains_rule(&rule.id) {
          continue;
        }
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        if suppression.check_ignore_all() {
          break;
        }
        if suppression.contains_rule(&rule.id) {
          continue;
        }
        if rule.fix.is_none() || !separate_fix {
          let matches = result.matches.entry(idx).or_default();
          matches.push(ret);
        } else {
          result.diffs.push((idx, ret));
        }
      }
    }
    result
  }

  pub fn get_rule(&self, idx: usize) -> &RuleConfig<L> {
    self.rules[idx]
  }

  pub fn all_kinds(&self) -> BitSet {
    (0..self.kind_rule_mapping.len()).collect()
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

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;
  use crate::SerializableRuleConfig;

  fn create_rule() -> RuleConfig<TypeScript> {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {pattern: 'console.log($A)'}
language: Tsx",
    )
    .expect("parse");
    RuleConfig::try_from(rule, &Default::default()).expect("work")
  }

  #[test]
  fn test_ignore_node() {
    let source = r#"
    // ast-grep-ignore
    console.log('ignored all')
    console.log('no ignore')
    // ast-grep-ignore: test
    console.log('ignore one')
    // ast-grep-ignore: not-test
    console.log('ignore another')
    // ast-grep-ignore: not-test, test
    console.log('multiple ignore')
    "#;
    let root = TypeScript::Tsx.ast_grep(source);
    let rule = create_rule();
    let rules = vec![&rule];
    let scan = CombinedScan::new(rules);
    let bits = scan.find(&root);
    let scanned = scan.scan(&root, bits, false);
    let matches = &scanned.matches[&0];
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].text(), "console.log('no ignore')");
    assert_eq!(matches[1].text(), "console.log('ignore another')");
  }
}
