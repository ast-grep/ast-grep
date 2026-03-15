use super::parameterized_util::{validate_utility_arguments, validate_utility_id};
use super::referent_rule::{GlobalRules, ReferentRuleError, RuleRegistration};
use crate::check_var::CheckHint;
use crate::maybe::Maybe;
use crate::rule::{self, Rule, RuleSerializeError, SerializableMatches, SerializableRule};
use crate::rule_core::{RuleCoreError, SerializableRuleCore};
use crate::transform::Trans;
use ast_grep_core::meta_var::MetaVariable;

use ast_grep_core::language::Language;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableGlobalRule<L: Language> {
  #[serde(flatten)]
  pub core: SerializableRuleCore,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Optional parameter names for a parameterized global utility rule.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub arguments: Option<Vec<String>>,
  /// Specify the language to parse and the file extension to include in matching.
  pub language: L,
}

struct ParsedGlobalRule<L: Language> {
  lang: L,
  core: SerializableRuleCore,
  params: Vec<String>,
}

fn into_map<L: Language>(
  rules: Vec<SerializableGlobalRule<L>>,
) -> Result<HashMap<String, ParsedGlobalRule<L>>, RuleSerializeError> {
  let mut parsed = HashMap::new();
  for rule in rules {
    validate_utility_id(&rule.id)?;
    let params = rule.arguments.unwrap_or_default();
    validate_utility_arguments(&rule.id, &params)?;
    if parsed.contains_key(&rule.id) {
      return Err(RuleSerializeError::MatchesReference(
        ReferentRuleError::DuplicateRule(rule.id),
      ));
    }
    parsed.insert(
      rule.id,
      ParsedGlobalRule {
        lang: rule.language,
        core: rule.core,
        params,
      },
    );
  }
  Ok(parsed)
}

type OrderResult<T> = Result<T, String>;

/// A struct to store information to deserialize rules.
#[derive(Clone)]
pub struct DeserializeEnv<L: Language> {
  /// registration for global utility rules and local utility rules.
  pub(crate) registration: RuleRegistration,
  /// current rules' language
  pub(crate) lang: L,
}

trait DependentRule: Sized {
  fn visit_dependency<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>) -> OrderResult<()>;
}

impl DependentRule for SerializableRule {
  fn visit_dependency<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>) -> OrderResult<()> {
    visit_dependent_rule_ids_with_params(self, sorter, None)
  }
}

impl<L: Language> DependentRule for ParsedGlobalRule<L> {
  fn visit_dependency<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>) -> OrderResult<()> {
    visit_dependent_rule_ids_with_params(&self.core.rule, sorter, Some(&self.params))?;
    if let Some(constraints) = &self.core.constraints {
      for rule in constraints.values() {
        visit_dependent_rule_ids_with_params(rule, sorter, Some(&self.params))?;
      }
    }
    if let Some(utils) = &self.core.utils {
      for rule in utils.values() {
        visit_dependent_rule_ids_with_params(rule, sorter, Some(&self.params))?;
      }
    }
    Ok(())
  }
}

impl DependentRule for Trans<MetaVariable> {
  fn visit_dependency<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>) -> OrderResult<()> {
    let used_var = self.used_vars();
    sorter.visit(used_var)
  }
}

/// A struct to topological sort rules
/// it is used to report cyclic dependency errors in rules/transformation
struct TopologicalSort<'a, T: DependentRule> {
  maps: &'a HashMap<String, T>,
  order: Vec<&'a str>,
  // bool stands for if the rule has completed visit
  seen: HashMap<&'a str, bool>,
}

impl<'a, T: DependentRule> TopologicalSort<'a, T> {
  fn get_order(maps: &HashMap<String, T>) -> OrderResult<Vec<&str>> {
    let mut top_sort = TopologicalSort::new(maps);
    for key in maps.keys() {
      top_sort.visit(key)?;
    }
    Ok(top_sort.order)
  }

  fn new(maps: &'a HashMap<String, T>) -> Self {
    Self {
      maps,
      order: vec![],
      seen: HashMap::new(),
    }
  }

  fn visit(&mut self, key: &'a str) -> OrderResult<()> {
    if let Some(&completed) = self.seen.get(key) {
      // if the rule has been seen but not completed
      // it means we have a cyclic dependency and report an error here
      return if completed {
        Ok(())
      } else {
        Err(key.to_string())
      };
    }
    let Some(item) = self.maps.get(key) else {
      // key can be found elsewhere
      // e.g. if key is rule_id
      // if rule_id not found in global, it can be a local rule
      // if rule_id not found in local, it can be a global rule
      // TODO: add check here and return Err if rule not found
      return Ok(());
    };
    // mark the id as seen but not completed
    self.seen.insert(key, false);
    item.visit_dependency(self)?;
    // mark the id as seen and completed
    self.seen.insert(key, true);
    self.order.push(key);
    Ok(())
  }
}

fn visit_dependent_rule_ids_with_params<'a, T: DependentRule>(
  rule: &'a SerializableRule,
  sort: &mut TopologicalSort<'a, T>,
  params: Option<&[String]>,
) -> OrderResult<()> {
  // handle all composite rule here
  if let Maybe::Present(matches) = &rule.matches {
    match matches {
      SerializableMatches::Id(id) => {
        if !params.is_some_and(|params| params.contains(id)) {
          sort.visit(id)?;
        }
      }
      SerializableMatches::Call(call) => {
        for (callee, args) in call.iter() {
          sort.visit(callee)?;
          for arg in args.values() {
            visit_dependent_rule_ids_with_params(arg, sort, params)?;
          }
        }
      }
    }
  }
  if let Maybe::Present(all) = &rule.all {
    for sub in all {
      visit_dependent_rule_ids_with_params(sub, sort, params)?;
    }
  }
  if let Maybe::Present(any) = &rule.any {
    for sub in any {
      visit_dependent_rule_ids_with_params(sub, sort, params)?;
    }
  }
  if let Maybe::Present(not) = &rule.not {
    visit_dependent_rule_ids_with_params(not, sort, params)?;
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

  pub(crate) fn from_registration(lang: L, registration: RuleRegistration) -> Self {
    Self { registration, lang }
  }

  /// register utils rule in the DeserializeEnv for later usage.
  /// N.B. This function will manage the util registration order
  /// by their dependency. `potential_kinds` need ordered insertion.
  pub fn with_utils(
    self,
    utils: &HashMap<String, SerializableRule>,
  ) -> Result<Self, RuleSerializeError> {
    validate_local_utils(utils)?;
    let order = TopologicalSort::get_order(utils)
      .map_err(ReferentRuleError::CyclicRule)
      .map_err(RuleSerializeError::MatchesReference)?;
    for id in order {
      let util = utils.get(id).expect("must exist");
      let rule = self.deserialize_rule(util.clone())?;
      self
        .registration
        .insert_local(id, rule)
        .map_err(RuleSerializeError::MatchesReference)?;
    }
    Ok(self)
  }

  /// register global utils rule discovered in the config.
  pub fn parse_global_utils(
    utils: Vec<SerializableGlobalRule<L>>,
  ) -> Result<GlobalRules, RuleCoreError> {
    let registration = GlobalRules::default();
    let utils = into_map(utils)?;
    let order = TopologicalSort::get_order(&utils)
      .map_err(ReferentRuleError::CyclicRule)
      .map_err(RuleSerializeError::from)?;
    for id in order {
      let parsed = utils.get(id).expect("must exist");
      let params = (!parsed.params.is_empty()).then(|| parsed.params.iter().cloned().collect());
      let env_registration = RuleRegistration::from_globals(&registration, params);
      let env = DeserializeEnv::from_registration(parsed.lang.clone(), env_registration);
      let matcher = parsed.core.get_matcher_with_hint(env, CheckHint::Global)?;
      registration
        .insert(
          id,
          matcher,
          (!parsed.params.is_empty()).then(|| parsed.params.clone()),
        )
        .map_err(RuleSerializeError::MatchesReference)?;
    }
    Ok(registration)
  }

  pub fn deserialize_rule(&self, serialized: SerializableRule) -> Result<Rule, RuleSerializeError> {
    rule::deserialize_rule(serialized, self)
  }

  pub(crate) fn get_transform_order<'a>(
    &self,
    trans: &'a HashMap<String, Trans<MetaVariable>>,
  ) -> Result<Vec<&'a str>, String> {
    TopologicalSort::get_order(trans)
  }

  pub fn with_globals(self, globals: &GlobalRules) -> Self {
    Self {
      registration: RuleRegistration::from_globals(globals, None),
      lang: self.lang,
    }
  }
}

fn validate_local_utils(
  utils: &HashMap<String, SerializableRule>,
) -> Result<(), RuleSerializeError> {
  for raw_id in utils.keys() {
    // Local utils currently only support plain ids. If signature-style local
    // declarations come back in the future, this is the choke point to relax.
    validate_utility_id(raw_id)?;
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript;
  use crate::{from_str, Rule};
  use ast_grep_core::tree_sitter::LanguageExt;
  use ast_grep_core::Matcher;

  type Result<T> = std::result::Result<T, RuleSerializeError>;

  fn get_dependent_utils() -> Result<(Rule, DeserializeEnv<TypeScript>)> {
    let utils = from_str(
      "
accessor-name:
  matches: member-name
  regex: whatever
member-name:
  kind: identifier
",
    )
    .expect("failed to parse utils");
    let env = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils)?;
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
    )
    .expect("failed to parse utils");
    // should not panic
    DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils)?;
    Ok(())
  }

  #[test]
  fn test_using_cyclic_local() -> Result<()> {
    let utils = from_str(
      "
local-rule:
  matches: local-rule
",
    )
    .expect("failed to parse utils");
    let ret = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils);
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
    )
    .expect("failed to parse utils");
    let ret = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils);
    assert!(ret.is_err());
    Ok(())
  }

  #[test]
  fn test_cyclic_not() -> Result<()> {
    let utils = from_str(
      "
local-rule-a:
  not: {matches: local-rule-b}
local-rule-b:
  matches: local-rule-a",
    )
    .expect("failed to parse utils");
    let ret = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils);
    assert!(matches!(
      ret,
      Err(RuleSerializeError::MatchesReference(
        ReferentRuleError::CyclicRule(_)
      ))
    ));
    Ok(())
  }

  #[test]
  fn test_local_utils_reject_reserved_id_chars() {
    let utils = from_str(
      r"
wrap(BODY):
  kind: number
",
    )
    .expect("failed to parse utils");
    let ret = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils);
    assert!(matches!(
      ret,
      Err(RuleSerializeError::InvalidUtils(
        rule::ParameterizedUtilError::InvalidUtilityId(signature)
      )) if signature == "wrap(BODY)"
    ));
  }

  #[test]
  fn test_invalid_global_utility_id() {
    let globals: Vec<SerializableGlobalRule<TypeScript>> = from_str(
      r"
- id: wrap()
  language: Tsx
  rule:
    kind: number
",
    )
    .expect("failed to parse globals");
    let ret = DeserializeEnv::parse_global_utils(globals);
    assert!(matches!(
      ret,
      Err(RuleCoreError::Rule(RuleSerializeError::InvalidUtils(
        rule::ParameterizedUtilError::InvalidUtilityId(signature)
      ))) if signature == "wrap()"
    ));
  }
}
