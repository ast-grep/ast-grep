use crate::maybe::Maybe;
use crate::referent_rule::{GlobalRules, RuleRegistration};
use crate::rule::{deserialize_rule, RuleSerializeError, SerializableRule};
use crate::rule_config::{RuleConfigError, SerializableRuleCore};

use ast_grep_core::language::Language;

use std::collections::{HashMap, HashSet};

pub struct DeserializeEnv<L: Language> {
  pub(crate) registration: RuleRegistration<L>,
  pub(crate) lang: L,
}

struct TopologicalSort<'a> {
  utils: &'a HashMap<String, SerializableRule>,
  order: Vec<&'a String>,
  seen: HashSet<&'a String>,
}

impl<'a> TopologicalSort<'a> {
  fn new(utils: &'a HashMap<String, SerializableRule>) -> Self {
    Self {
      utils,
      order: vec![],
      seen: HashSet::new(),
    }
  }

  fn visit(&mut self, rule_id: &'a String) {
    if self.seen.contains(rule_id) {
      return;
    }
    let rule = self
      .utils
      .get(rule_id)
      .expect("rule_id must exist in utils");
    visit_dependent_rule_ids(rule, self);
    self.seen.insert(rule_id);
    self.order.push(rule_id);
  }
}

/// NOTE: this function only needs to handle rules used in potential_kinds
fn visit_dependent_rule_ids<'a>(rule: &'a SerializableRule, sort: &mut TopologicalSort<'a>) {
  // handle all composite rule here
  if let Maybe::Present(matches) = &rule.matches {
    sort.visit(matches);
  }
  if let Maybe::Present(all) = &rule.all {
    for sub in all {
      visit_dependent_rule_ids(sub, sort);
    }
  }
  if let Maybe::Present(any) = &rule.any {
    for sub in any {
      visit_dependent_rule_ids(sub, sort);
    }
  }
  if let Maybe::Present(_not) = &rule.not {
    // TODO: check cyclic here
  }
}

fn get_order(utils: &HashMap<String, SerializableRule>) -> Vec<&String> {
  let mut top_sort = TopologicalSort::new(utils);
  for rule_id in utils.keys() {
    top_sort.visit(rule_id);
  }
  top_sort.order
}

impl<L: Language> DeserializeEnv<L> {
  pub fn new(lang: L) -> Self {
    Self {
      registration: Default::default(),
      lang,
    }
  }

  /// register utils rule in the DeserializeEnv for later usage.
  /// N.B. this function will manage the util registration order
  /// by their dependency. potential_kinds need orderd insertion.
  pub fn register_local_utils(
    self,
    utils: &HashMap<String, SerializableRule>,
  ) -> Result<Self, RuleSerializeError> {
    let order = get_order(utils);
    for id in order {
      let rule = utils.get(id).expect("must exist");
      let rule = deserialize_rule(rule.clone(), &self)?;
      self.registration.insert_local(id, rule)?;
    }
    Ok(self)
  }

  pub fn parse_global_utils(
    utils: Vec<SerializableRuleCore<L>>,
  ) -> Result<GlobalRules<L>, RuleConfigError> {
    let registration = GlobalRules::default();
    for rule in utils {
      let matcher = rule.get_matcher(&registration)?;
      registration
        .insert(&rule.id, matcher)
        .map_err(RuleSerializeError::MatchesRefrence)?;
    }
    Ok(registration)
  }

  pub fn with_globals(self, globals: &GlobalRules<L>) -> Self {
    Self {
      registration: RuleRegistration::from_globals(globals),
      lang: self.lang,
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript;
  use crate::{from_str, Rule};
  use ast_grep_core::Matcher;

  fn get_dependent_utils() -> (Rule<TypeScript>, DeserializeEnv<TypeScript>) {
    let utils = from_str(
      "
accessor-name:
  matches: member-name
  regex: whatever
member-name:
  kind: identifier
",
    )
    .unwrap();
    let env = DeserializeEnv::new(TypeScript::Tsx)
      .register_local_utils(&utils)
      .unwrap();
    assert_eq!(utils.keys().count(), 2);
    let rule = from_str("matches: accessor-name").unwrap();
    (
      deserialize_rule(rule, &env).unwrap(),
      env, // env is required for weak ref
    )
  }

  #[test]
  fn test_local_util_matches() {
    let (rule, _env) = get_dependent_utils();
    let grep = TypeScript::Tsx.ast_grep("whatever");
    assert!(grep.root().find(rule).is_some());
  }

  #[test]
  fn test_local_util_kinds() {
    // run multiple times to avoid accidental working order due to HashMap randomness
    for _ in 0..10 {
      let (rule, _env) = get_dependent_utils();
      assert!(rule.potential_kinds().is_some());
    }
  }
}
