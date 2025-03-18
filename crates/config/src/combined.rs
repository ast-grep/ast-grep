use crate::{RuleConfig, SerializableRule, SerializableRuleConfig, SerializableRuleCore, Severity};

use ast_grep_core::language::Language;
use ast_grep_core::matcher::{Matcher, MatcherExt};
use ast_grep_core::{AstGrep, Doc, Node, NodeMatch};

use std::collections::{HashMap, HashSet};

pub struct ScanResult<'t, 'r, D: Doc, L: Language> {
  pub diffs: Vec<(&'r RuleConfig<L>, NodeMatch<'t, D>)>,
  pub matches: Vec<(&'r RuleConfig<L>, Vec<NodeMatch<'t, D>>)>,
}

/// store the index to the rule and the matched node
/// it will be converted to ScanResult by resolving the rule
struct ScanResultInner<'t, D: Doc> {
  diffs: Vec<(usize, NodeMatch<'t, D>)>,
  matches: HashMap<usize, Vec<NodeMatch<'t, D>>>,
  unused_suppressions: Vec<NodeMatch<'t, D>>,
}

impl<'t, D: Doc> ScanResultInner<'t, D> {
  pub fn into_result<'r, L: Language>(
    self,
    combined: &CombinedScan<'r, L>,
    separate_fix: bool,
  ) -> ScanResult<'t, 'r, D, L> {
    let mut diffs: Vec<_> = self
      .diffs
      .into_iter()
      .map(|(idx, nm)| (combined.get_rule(idx), nm))
      .collect();
    let mut matches: Vec<_> = self
      .matches
      .into_iter()
      .map(|(idx, nms)| (combined.get_rule(idx), nms))
      .collect();
    if let Some(rule) = combined.unused_suppression_rule {
      if separate_fix {
        diffs.extend(self.unused_suppressions.into_iter().map(|nm| (rule, nm)));
        diffs.sort_unstable_by_key(|(_, nm)| nm.range().start);
      } else if !self.unused_suppressions.is_empty() {
        // do not push empty suppression to matches
        let mut supprs = self.unused_suppressions;
        supprs.sort_unstable_by_key(|nm| nm.range().start);
        matches.push((rule, supprs));
      }
    }
    ScanResult { diffs, matches }
  }
}

struct Suppressions(HashMap<usize, Suppression>);
impl Suppressions {
  fn collect<D: Doc>(&mut self, node: &Node<D>) {
    if !node.kind().contains("comment") || !node.text().contains(IGNORE_TEXT) {
      return;
    }
    let line = node.start_pos().line();
    let suppress_next_line = if let Some(prev) = node.prev() {
      prev.start_pos().line() != line
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
    let line = node.start_pos().line();
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

impl MaySuppressed<'_> {
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

/// A struct to group all rules according to their potential kinds.
/// This can greatly reduce traversal times and skip unmatchable rules.
/// Rules are referenced by their index in the rules vector.
pub struct CombinedScan<'r, L: Language> {
  rules: Vec<&'r RuleConfig<L>>,
  /// a vec of vec, mapping from kind to a list of rule index
  kind_rule_mapping: Vec<Vec<usize>>,
  /// a rule for unused_suppressions
  unused_suppression_rule: Option<&'r RuleConfig<L>>,
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
      unused_suppression_rule: None,
    }
  }

  pub fn set_unused_suppression_rule(&mut self, rule: &'r RuleConfig<L>) {
    if matches!(rule.severity, Severity::Off) {
      return;
    }
    self.unused_suppression_rule = Some(rule);
  }

  pub fn scan<'a, D>(&self, root: &'a AstGrep<D>, separate_fix: bool) -> ScanResult<'a, '_, D, L>
  where
    D: Doc<Lang = L>,
  {
    let mut result = ScanResultInner {
      diffs: vec![],
      matches: HashMap::new(),
      unused_suppressions: vec![],
    };
    let mut suppressions = Suppressions(HashMap::new());
    for node in root.root().dfs() {
      suppressions.collect(&node);
    }
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
    result.into_result(self, separate_fix)
  }

  pub fn get_rule(&self, idx: usize) -> &'r RuleConfig<L> {
    self.rules[idx]
  }

  pub fn unused_config(severity: Severity, lang: L) -> RuleConfig<L> {
    let rule: SerializableRule = crate::from_str(r#"{"any": []}"#).unwrap();
    let core = SerializableRuleCore {
      rule,
      constraints: None,
      fix: crate::from_str(r#"''"#).unwrap(),
      transform: None,
      utils: None,
    };
    let config = SerializableRuleConfig {
      core,
      id: "unused-suppression".to_string(),
      severity,
      files: None,
      ignores: None,
      language: lang,
      message: "Unused 'ast-grep-ignore' directive.".into(),
      metadata: None,
      note: None,
      rewriters: None,
      url: None,
    };
    RuleConfig::try_from(config, &Default::default()).unwrap()
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
    let scanned = scan.scan(&root, false);
    let matches = &scanned.matches[0];
    assert_eq!(matches.1.len(), 2);
    assert_eq!(matches.1[0].text(), "console.log('no ignore')");
    assert_eq!(matches.1[1].text(), "console.log('ignore another')");
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
    let scanned = scan.scan(&root, false);
    let matches = &scanned.matches[0];
    assert_eq!(matches.1.len(), 2);
    assert_eq!(matches.1[0].text(), "console.log('no ignore')");
    assert_eq!(matches.1[1].text(), "console.log('ignore another')");
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
    let mut scan = CombinedScan::new(rules);
    scan.set_unused_suppression_rule(&rule);
    let scanned = scan.scan(&root, false);
    assert_eq!(scanned.matches.len(), 2);
    let unused = &scanned.matches[1];
    assert_eq!(unused.1.len(), 1);
    assert_eq!(unused.1[0].text(), "// ast-grep-ignore: test");
  }
}
