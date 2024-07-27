use crate::RuleConfig;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Doc, Matcher, Node, NodeMatch};

use bit_set::BitSet;
use std::collections::{HashMap, HashSet};

pub struct ScanResult<'r, D: Doc> {
  pub diffs: Vec<(usize, NodeMatch<'r, D>)>,
  pub matches: HashMap<usize, Vec<NodeMatch<'r, D>>>,
}

struct Suppression {
  is_used: bool,
  /// None = suppress all
  suppressed: Option<HashSet<String>>,
  line_num: usize,
}

enum MaySuppressed<'a> {
  Yes(&'a mut Suppression),
  No,
}

impl<'a> MaySuppressed<'a> {
  fn is_suppressed(&mut self, rule_id: &str) -> bool {
    let suppression = match self {
      MaySuppressed::No => return false,
      MaySuppressed::Yes(s) => s,
    };
    if let Some(set) = &mut suppression.suppressed {
      if set.contains(rule_id) {
        suppression.is_used = true;
        true
      } else {
        false
      }
    } else {
      suppression.is_used = true;
      true
    }
  }
}

const IGNORE_TEXT: &str = "ast-grep-ignore";

impl Suppression {
  fn detect<D: Doc>(node: &Node<D>) -> Option<Suppression> {
    if !node.kind().contains("comment") || !node.text().contains(IGNORE_TEXT) {
      return None;
    }
    let line = node.start_pos().0;
    let suppress_next_line = if let Some(prev) = node.prev() {
      prev.start_pos().0 != line
    } else {
      true
    };
    Some(Suppression {
      is_used: false,
      suppressed: parse_suppression_set(&node.text()),
      line_num: if suppress_next_line { line + 1 } else { line },
    })
  }
}

pub struct PreScan {
  pub hit_set: BitSet,
  suppressions: Vec<Suppression>,
}

impl PreScan {
  fn check_suppression<D: Doc>(&mut self, node: &Node<D>) -> MaySuppressed {
    let line = node.start_pos().0;
    for suppression in &mut self.suppressions {
      if suppression.line_num == line {
        return MaySuppressed::Yes(suppression);
      }
    }
    MaySuppressed::No
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

  pub fn new_find<D>(&self, root: &AstGrep<D>) -> PreScan
  where
    D: Doc<Lang = L>,
  {
    let mut hit = BitSet::new();
    let mut suppressions = vec![];
    for node in root.root().dfs() {
      suppressions.extend(Suppression::detect(&node));
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

  pub fn new_scan<'a, D>(
    &self,
    root: &'a AstGrep<D>,
    mut pre: PreScan,
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
      let hit_set = pre.hit_set.clone();
      let mut suppression = pre.check_suppression(&node);
      for &idx in rule_idx {
        if !hit_set.contains(idx) {
          continue;
        }
        let rule = &self.rules[idx];
        // if suppression.is_suppressed(&rule.id) {
        //   continue;
        // }
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        // if suppression.check_ignore_all() {
        //   break;
        // }
        if suppression.is_suppressed(&rule.id) {
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
    // previous node on the same line
    if prev.start_pos().0 == node.start_pos().0 {
      let Some(n) = node.parent() else {
        return NoSuppression;
      };
      node = n;
    // previous node on the previous line
    } else if prev.start_pos().0 + 1 == node.start_pos().0 {
      return parse_suppression(&prev.text());
    } else {
      return NoSuppression;
    }
  }
}

fn parse_suppression_set(text: &str) -> Option<HashSet<String>> {
  let (_, after) = text.trim().split_once("ast-grep-ignore")?;
  let after = after.trim();
  if after.is_empty() {
    return None;
  }
  let (_, rules) = after.split_once(':')?;
  let set = rules.split(',').map(|r| r.trim().to_string()).collect();
  Some(set)
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
    let pre = scan.new_find(&root);
    assert_eq!(pre.suppressions.len(), 4);
    let scanned = scan.new_scan(&root, pre, false);
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
    let pre = scan.new_find(&root);
    assert_eq!(pre.suppressions.len(), 4);
    let scanned = scan.new_scan(&root, pre, false);
    let matches = &scanned.matches[&0];
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].text(), "console.log('no ignore')");
    assert_eq!(matches[1].text(), "console.log('ignore another')");
  }
}
