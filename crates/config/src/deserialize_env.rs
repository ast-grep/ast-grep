use crate::maybe::Maybe;
use crate::referent_rule::{GlobalRules, ReferentRuleError, RuleRegistration};
use crate::rule::{self, Rule, RuleSerializeError, SerializableRule};
use crate::rule_config::{
  into_map, RuleConfigError, SerializableRuleConfigCore, SerializableRuleCore,
};

use ast_grep_core::language::Language;

use std::collections::HashMap;

type OrderResult<T> = Result<T, ReferentRuleError>;

pub struct DeserializeEnv<L: Language> {
  pub(crate) registration: RuleRegistration<L>,
  pub(crate) lang: L,
}

trait DepedentRule: Sized {
  fn visit_dependent_rules<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>)
    -> OrderResult<()>;
}

impl DepedentRule for SerializableRule {
  fn visit_dependent_rules<'a>(
    &'a self,
    sorter: &mut TopologicalSort<'a, Self>,
  ) -> OrderResult<()> {
    visit_dependent_rule_ids(self, sorter)
  }
}

impl<L: Language> DepedentRule for SerializableRuleCore<L> {
  fn visit_dependent_rules<'a>(
    &'a self,
    sorter: &mut TopologicalSort<'a, Self>,
  ) -> OrderResult<()> {
    visit_dependent_rule_ids(&self.rule, sorter)
  }
}

struct TopologicalSort<'a, T: DepedentRule> {
  utils: &'a HashMap<String, T>,
  order: Vec<&'a String>,
  // bool stands for if the rule has completed visit
  seen: HashMap<&'a String, bool>,
}

impl<'a, T: DepedentRule> TopologicalSort<'a, T> {
  fn get_order(utils: &HashMap<String, T>) -> OrderResult<Vec<&String>> {
    let mut top_sort = TopologicalSort::new(utils);
    for rule_id in utils.keys() {
      top_sort.visit(rule_id)?;
    }
    Ok(top_sort.order)
  }

  fn new(utils: &'a HashMap<String, T>) -> Self {
    Self {
      utils,
      order: vec![],
      seen: HashMap::new(),
    }
  }

  fn visit(&mut self, rule_id: &'a String) -> OrderResult<()> {
    if let Some(&completed) = self.seen.get(rule_id) {
      // if the rule has been seen but not completed
      // it means we have a cyclic dependency and report an error here
      return if completed {
        Ok(())
      } else {
        Err(ReferentRuleError::CyclicRule)
      };
    }
    let Some(rule) = self.utils.get(rule_id) else {
      // if rule_id not found in global, it can be a local rule
      // if rule_id not found in local, it can be a global rule
      // TODO: add check here and return Err if rule not found
      return Ok(());
    };
    // mark the id as seen but not completed
    self.seen.insert(rule_id, false);
    rule.visit_dependent_rules(self)?;
    // mark the id as seen and completed
    self.seen.insert(rule_id, true);
    self.order.push(rule_id);
    Ok(())
  }
}

fn visit_dependent_rule_ids<'a, T: DepedentRule>(
  rule: &'a SerializableRule,
  sort: &mut TopologicalSort<'a, T>,
) -> OrderResult<()> {
  // handle all composite rule here
  if let Maybe::Present(matches) = &rule.matches {
    sort.visit(matches)?;
  }
  if let Maybe::Present(all) = &rule.all {
    for sub in all {
      visit_dependent_rule_ids(sub, sort)?;
    }
  }
  if let Maybe::Present(any) = &rule.any {
    for sub in any {
      visit_dependent_rule_ids(sub, sort)?;
    }
  }
  if let Maybe::Present(_not) = &rule.not {
    // TODO: check cyclic here
  }
  Ok(())
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
    let order = TopologicalSort::get_order(utils)?;
    for id in order {
      let rule = utils.get(id).expect("must exist");
      let rule = self.deserialize_rule(rule.clone())?;
      self.registration.insert_local(id, rule)?;
    }
    Ok(self)
  }

  pub fn parse_global_utils(
    utils: Vec<SerializableRuleConfigCore<L>>,
  ) -> Result<GlobalRules<L>, RuleConfigError> {
    let registration = GlobalRules::default();
    let utils = into_map(utils);
    let order = TopologicalSort::get_order(&utils).map_err(RuleSerializeError::from)?;
    for id in order {
      let rule = utils.get(id).expect("must exist");
      let matcher = rule.get_matcher(&registration)?;
      registration
        .insert(id, matcher)
        .map_err(RuleSerializeError::MatchesRefrence)?;
    }
    Ok(registration)
  }

  pub fn deserialize_rule(
    &self,
    serialized: SerializableRule,
  ) -> Result<Rule<L>, RuleSerializeError> {
    rule::deserialize_rule(serialized, self)
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
  use anyhow::Result;
  use ast_grep_core::Matcher;

  fn get_dependent_utils() -> Result<(Rule<TypeScript>, DeserializeEnv<TypeScript>)> {
    let utils = from_str(
      "
accessor-name:
  matches: member-name
  regex: whatever
member-name:
  kind: identifier
",
    )?;
    let env = DeserializeEnv::new(TypeScript::Tsx).register_local_utils(&utils)?;
    assert_eq!(utils.keys().count(), 2);
    let rule = from_str("matches: accessor-name").unwrap();
    Ok((
      env.deserialize_rule(rule).unwrap(),
      env, // env is required for weak ref
    ))
  }

  #[test]
  fn test_local_util_matches() -> Result<()> {
    let (rule, _env) = get_dependent_utils()?;
    let grep = TypeScript::Tsx.ast_grep("whatever");
    assert!(grep.root().find(rule).is_some());
    Ok(())
  }

  #[test]
  fn test_local_util_kinds() -> Result<()> {
    // run multiple times to avoid accidental working order due to HashMap randomness
    for _ in 0..10 {
      let (rule, _env) = get_dependent_utils()?;
      assert!(rule.potential_kinds().is_some());
    }
    Ok(())
  }

  #[test]
  fn test_using_global_rule_in_local() -> Result<()> {
    let utils = from_str(
      "
local-rule:
  matches: global-rule
",
    )?;
    // should not panic
    DeserializeEnv::new(TypeScript::Tsx).register_local_utils(&utils)?;
    Ok(())
  }

  #[test]
  fn test_using_cyclic_local() -> Result<()> {
    let utils = from_str(
      "
local-rule:
  matches: local-rule
",
    )?;
    let ret = DeserializeEnv::new(TypeScript::Tsx).register_local_utils(&utils);
    assert!(ret.is_err());
    Ok(())
  }

  #[test]
  fn test_using_transitive_cycle() -> Result<()> {
    let utils = from_str(
      "
local-rule-a:
  matches: local-rule-b
local-rule-b:
  all:
    - matches: local-rule-c
local-rule-c:
  any:
    - matches: local-rule-a
",
    )?;
    let ret = DeserializeEnv::new(TypeScript::Tsx).register_local_utils(&utils);
    assert!(ret.is_err());
    Ok(())
  }
}
