use crate::RuleConfig;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Doc, Matcher, Node, NodeMatch};

use bit_set::BitSet;
use std::collections::{HashMap, HashSet};

pub struct ScanResult<'r, D: Doc> {
  pub diffs: Vec<(usize, NodeMatch<'r, D>)>,
  pub matches: HashMap<usize, Vec<NodeMatch<'r, D>>>,
  pub unused_suppressions: Vec<NodeMatch<'r, D>>,
}

struct Suppressions(HashMap<usize, Suppression>);
impl Suppressions {
  fn collect<D: Doc>(&mut self, node: &Node<D>) {
    if !node.kind().contains("comment") || !node.text().contains(IGNORE_TEXT) {
      return;
    }
    let line = node.start_pos().row();
    let suppress_next_line = if let Some(prev) = node.prev() {
      prev.start_pos().row() != line
    } else {
      true
    };
    let key = if suppress_next_line { line + 1 } else { line };
    self.0.insert(
      key,
      Suppression {
        suppressed: parse_suppression_set(&node.text()),
        node_id: node.node_id(),
      },
    );
  }

  fn suppression_ids(&self) -> HashSet<usize> {
    self.0.values().map(|s| s.node_id).collect()
  }

  fn check_suppression<D: Doc>(&mut self, node: &Node<D>) -> MaySuppressed {
    let line = node.start_pos().row();
    if let Some(sup) = self.0.get_mut(&line) {
      MaySuppressed::Yes(sup)
    } else {
      MaySuppressed::No
    }
  }
}

struct Suppression {
  /// None = suppress all
  suppressed: Option<HashSet<String>>,
  node_id: usize,
}

enum MaySuppressed<'a> {
  Yes(&'a Suppression),
  No,
}

impl<'a> MaySuppressed<'a> {
  fn suppressed_id(&self, rule_id: &str) -> Option<usize> {
    let suppression = match self {
      MaySuppressed::No => return None,
      MaySuppressed::Yes(s) => s,
    };
    if let Some(set) = &suppression.suppressed {
      if set.contains(rule_id) {
        Some(suppression.node_id)
      } else {
        None
      }
    } else {
      Some(suppression.node_id)
    }
  }
}

const IGNORE_TEXT: &str = "ast-grep-ignore";

pub struct PreScan {
  hit_set: BitSet,
  suppressions: Suppressions,
}

impl PreScan {
  pub fn is_empty(&self) -> bool {
    self.hit_set.is_empty() && self.suppressions.0.is_empty()
  }
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
      let Some(kinds) = rule.matcher.potential_kinds() else {
        eprintln!("rule `{}` must have kind", &rule.id);
        continue;
      };
      for kind in &kinds {
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

  pub fn find<D>(&self, root: &AstGrep<D>) -> PreScan
  where
    D: Doc<Lang = L>,
  {
    let mut hit = BitSet::new();
    let mut suppressions = Suppressions(HashMap::new());
    for node in root.root().dfs() {
      suppressions.collect(&node);
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
    PreScan {
      hit_set: hit,
      suppressions,
    }
  }

  pub fn scan<'a, D>(
    &self,
    root: &'a AstGrep<D>,
    pre: PreScan,
    separate_fix: bool,
  ) -> ScanResult<'a, D>
  where
    D: Doc<Lang = L>,
  {
    let mut result = ScanResult {
      diffs: vec![],
      matches: HashMap::new(),
      unused_suppressions: vec![],
    };
    let PreScan {
      hit_set,
      mut suppressions,
    } = pre;
    let mut suppression_ids = suppressions.suppression_ids();
    let mut suppression_nodes = HashMap::new();
    for node in root.root().dfs() {
      if suppression_ids.contains(&node.node_id()) {
        suppression_nodes.insert(node.node_id(), node.clone());
      }
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      let suppression = suppressions.check_suppression(&node);
      for &idx in rule_idx {
        if !hit_set.contains(idx) {
          continue;
        }
        let rule = &self.rules[idx];
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        if let Some(id) = suppression.suppressed_id(&rule.id) {
          suppression_ids.remove(&id);
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
    result.unused_suppressions = suppression_nodes
      .into_values()
      .filter_map(|node| {
        let node_id = node.node_id();
        let unused = suppression_ids.contains(&node_id);
        if unused {
          Some(NodeMatch::from(node))
        } else {
          None
        }
      })
      .collect();
    result
  }

  pub fn get_rule(&self, idx: usize) -> &RuleConfig<L> {
    self.rules[idx]
  }
}

fn parse_suppression_set(text: &str) -> Option<HashSet<String>> {
  let (_, after) = text.trim().split_once(IGNORE_TEXT)?;
  let after = after.trim();
  if after.is_empty() {
    return None;
  }
  let (_, rules) = after.split_once(':')?;
  let set = rules.split(',').map(|r| r.trim().to_string()).collect();
  Some(set)
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
    let pre = scan.find(&root);
    assert_eq!(pre.suppressions.0.len(), 4);
    let scanned = scan.scan(&root, pre, false);
    let matches = &scanned.matches[&0];
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].text(), "console.log('no ignore')");
    assert_eq!(matches[1].text(), "console.log('ignore another')");
  }

  #[test]
  fn test_ignore_node_same_line() {
    let source = r#"
    console.log('ignored all') // ast-grep-ignore
    console.log('no ignore')
    console.log('ignore one') // ast-grep-ignore: test
    console.log('ignore another') // ast-grep-ignore: not-test
    console.log('multiple ignore') // ast-grep-ignore: not-test, test
    "#;
    let root = TypeScript::Tsx.ast_grep(source);
    let rule = create_rule();
    let rules = vec![&rule];
    let scan = CombinedScan::new(rules);
    let pre = scan.find(&root);
    assert_eq!(pre.suppressions.0.len(), 4);
    let scanned = scan.scan(&root, pre, false);
    let matches = &scanned.matches[&0];
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].text(), "console.log('no ignore')");
    assert_eq!(matches[1].text(), "console.log('ignore another')");
  }

  #[test]
  fn test_non_used_suppression() {
    let source = r#"
    console.log('no ignore')
    console.debug('not used') // ast-grep-ignore: test
    console.log('multiple ignore') // ast-grep-ignore: test
    "#;
    let root = TypeScript::Tsx.ast_grep(source);
    let rule = create_rule();
    let rules = vec![&rule];
    let scan = CombinedScan::new(rules);
    let pre = scan.find(&root);
    assert_eq!(pre.suppressions.0.len(), 2);
    let scanned = scan.scan(&root, pre, false);
    let unused = &scanned.unused_suppressions;
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].text(), "// ast-grep-ignore: test");
  }
}
