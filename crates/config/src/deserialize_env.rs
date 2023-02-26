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

struct TopologicalSort<'a, T: DependentRule> {
  utils: &'a HashMap<String, T>,
  order: Vec<&'a String>,
  seen: HashSet<&'a String>,
}

impl<'a, T: DependentRule> TopologicalSort<'a, T> {
  fn new(utils: &'a HashMap<String, T>) -> Self {
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
    rule.visit_dependent_rule_ids(self);
    self.seen.insert(rule_id);
    self.order.push(rule_id);
  }
}

trait DependentRule: Sized {
  /// NOTE: this function only needs to handle rules used in potential_kinds
  fn visit_dependent_rule_ids<'a>(&'a self, sort: &mut TopologicalSort<'a, Self>);
}

impl DependentRule for SerializableRule {
  fn visit_dependent_rule_ids<'a>(&'a self, sort: &mut TopologicalSort<'a, Self>) {
    // handle all composite rule here
    if let Maybe::Present(matches) = &self.matches {
      sort.visit(matches);
    }
    if let Maybe::Present(all) = &self.all {
      for sub in all {
        sub.visit_dependent_rule_ids(sort);
      }
    }
    if let Maybe::Present(any) = &self.any {
      for sub in any {
        sub.visit_dependent_rule_ids(sort);
      }
    }
    if let Maybe::Present(_not) = &self.not {
      // TODO: check cyclic here
    }
  }
}

fn get_order<T: DependentRule>(utils: &HashMap<String, T>) -> Vec<&String> {
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
  /// N.B. This function will manage the util registration order
  /// by their dependency. `potential_kinds` need ordered insertion.
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
