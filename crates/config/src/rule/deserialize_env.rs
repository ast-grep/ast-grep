use super::parameterized_util::{GlobalTemplate, UtilitySignature};
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
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableGlobalRule<L: Language> {
  #[serde(flatten)]
  pub core: SerializableRuleCore,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
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
    let signature = UtilitySignature::parse(&rule.id)?;
    if parsed.contains_key(&signature.name) {
      return Err(RuleSerializeError::MatchesReference(
        ReferentRuleError::DuplicateRule(signature.name),
      ));
    }
    parsed.insert(
      signature.name,
      ParsedGlobalRule {
        lang: rule.language,
        core: rule.core,
        params: signature.params,
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
  /// current parameter bindings allowed during deserialization
  current_params: Option<Arc<HashSet<String>>>,
}

trait DependentRule: Sized {
  fn visit_dependency<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>) -> OrderResult<()>;
}

trait DeclaredUtil {
  fn params(&self) -> &[String];
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
      for (id, rule) in utils {
        let params = UtilitySignature::parse(id)
          .map_err(|err| err.to_string())?
          .params;
        visit_dependent_rule_ids_with_params(rule, sorter, Some(&params))?;
      }
    }
    Ok(())
  }
}

impl<L: Language> DeclaredUtil for ParsedGlobalRule<L> {
  fn params(&self) -> &[String] {
    &self.params
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

fn register_ordered_utils<'a, T: DeclaredUtil, E>(
  order: Vec<&'a str>,
  utils: &'a HashMap<String, T>,
  mut register_rule: impl FnMut(&'a str, &'a T) -> Result<(), E>,
  mut register_template: impl FnMut(&'a str, &'a T) -> Result<(), E>,
) -> Result<(), E> {
  for id in order {
    let util = utils.get(id).expect("must exist");
    if util.params().is_empty() {
      register_rule(id, util)?;
    } else {
      register_template(id, util)?;
    }
  }
  Ok(())
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
        if !params.is_some_and(|params| params.iter().any(|param| param == id)) {
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
      current_params: None,
    }
  }

  pub(crate) fn from_registration(lang: L, registration: RuleRegistration) -> Self {
    Self {
      registration,
      lang,
      current_params: None,
    }
  }

  /// register utils rule in the DeserializeEnv for later usage.
  /// N.B. This function will manage the util registration order
  /// by their dependency. `potential_kinds` need ordered insertion.
  pub fn with_utils(
    self,
    utils: &HashMap<String, SerializableRule>,
  ) -> Result<Self, RuleSerializeError> {
    let parsed = parse_utils(utils)?;
    let order = TopologicalSort::get_order(&parsed)
      .map_err(ReferentRuleError::CyclicRule)
      .map_err(RuleSerializeError::MatchesReference)?;
    let env = self;
    register_ordered_utils(
      order,
      &parsed,
      |id, util| -> Result<(), RuleSerializeError> {
        let rule = env.deserialize_rule(util.body.clone())?;
        env
          .registration
          .insert_local(id, rule)
          .map_err(RuleSerializeError::MatchesReference)?;
        Ok(())
      },
      |id, util| -> Result<(), RuleSerializeError> {
        let params = util.params.iter().cloned().collect();
        let template = env
          .with_params(params)
          .deserialize_rule(util.body.clone())?;
        env
          .registration
          .insert_local_template(id, util.params.clone(), template)
          .map_err(RuleSerializeError::MatchesReference)?;
        Ok(())
      },
    )?;
    Ok(env)
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
    register_ordered_utils(
      order,
      &utils,
      |id, parsed| -> Result<(), RuleCoreError> {
        let env = DeserializeEnv::new(parsed.lang.clone()).with_globals(&registration);
        let matcher = parsed.core.get_matcher_with_hint(env, CheckHint::Global)?;
        registration
          .insert(id, matcher)
          .map_err(RuleSerializeError::MatchesReference)?;
        Ok(())
      },
      |id, parsed| -> Result<(), RuleCoreError> {
        let params = parsed.params.iter().cloned().collect();
        let env = DeserializeEnv::new(parsed.lang.clone()).with_globals(&registration);
        let matcher = parsed
          .core
          .get_matcher_with_hint(env.with_params(params), CheckHint::Global)?;
        registration
          .insert_template(id, GlobalTemplate::new(parsed.params.clone(), matcher))
          .map_err(RuleSerializeError::MatchesReference)?;
        Ok(())
      },
    )?;
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
      registration: RuleRegistration::from_globals(globals),
      lang: self.lang,
      current_params: self.current_params,
    }
  }

  pub(crate) fn current_params(&self) -> Option<&HashSet<String>> {
    self.current_params.as_deref()
  }

  pub(crate) fn has_current_param(&self, id: &str) -> bool {
    self
      .current_params
      .as_deref()
      .is_some_and(|params| params.contains(id))
  }

  pub(crate) fn with_params(&self, params: HashSet<String>) -> Self {
    let mut env = self.clone();
    env.current_params = Some(Arc::new(params));
    env
  }
}

type ParsedUtils = HashMap<String, ParsedUtil>;

struct ParsedUtil {
  params: Vec<String>,
  body: SerializableRule,
}

fn parse_utils(
  utils: &HashMap<String, SerializableRule>,
) -> Result<ParsedUtils, RuleSerializeError> {
  let mut parsed = HashMap::new();
  for (raw_id, body) in utils {
    let signature = UtilitySignature::parse(raw_id)?;
    if parsed.contains_key(&signature.name) {
      return Err(RuleSerializeError::MatchesReference(
        ReferentRuleError::DuplicateRule(signature.name),
      ));
    }
    parsed.insert(
      signature.name.clone(),
      ParsedUtil {
        params: signature.params,
        body: body.clone(),
      },
    );
  }
  Ok(parsed)
}

impl DependentRule for ParsedUtil {
  fn visit_dependency<'a>(&'a self, sorter: &mut TopologicalSort<'a, Self>) -> OrderResult<()> {
    visit_dependent_rule_ids_with_params(&self.body, sorter, Some(&self.params))
  }
}

impl DeclaredUtil for ParsedUtil {
  fn params(&self) -> &[String] {
    &self.params
  }
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
  fn test_parameterized_util_requires_all_args() -> Result<()> {
    let utils = from_str(
      r"
wrap(BODY):
  matches: BODY
",
    )
    .expect("failed to parse utils");
    let env = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils)?;
    let rule = from_str(
      r"
matches:
  wrap: {}
",
    )
    .expect("should parse rule");
    let ret = env.deserialize_rule(rule);
    assert!(matches!(
      ret,
      Err(RuleSerializeError::InvalidUtils(
        rule::ParameterizedUtilError::MissingUtilityArgument {
        callee,
        arg
      }))
      if callee == "wrap" && arg == "BODY"
    ));
    Ok(())
  }

  #[test]
  fn test_parameterized_util_rejects_unknown_args() -> Result<()> {
    let utils = from_str(
      r"
wrap(BODY):
  matches: BODY
",
    )
    .expect("failed to parse utils");
    let env = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils)?;
    let rule = from_str(
      r"
matches:
  wrap:
    OTHER:
      kind: number
    BODY:
      kind: number
",
    )
    .expect("should parse rule");
    let ret = env.deserialize_rule(rule);
    assert!(matches!(
      ret,
      Err(RuleSerializeError::InvalidUtils(
        rule::ParameterizedUtilError::UnknownUtilityArgument {
        callee,
        arg
      }))
      if callee == "wrap" && arg == "OTHER"
    ));
    Ok(())
  }

  #[test]
  fn test_parameterized_call_cycle_in_argument_rule() -> Result<()> {
    let utils = from_str(
      r"
RECUR(x):
  matches: x
",
    )
    .expect("failed to parse utils");
    let env = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils)?;
    let rule = from_str(
      r"
matches:
  RECUR:
    x:
      matches:
        RECUR:
          x:
            kind: number
",
    )
    .expect("should parse rule");
    let ret = env.deserialize_rule(rule);
    assert!(matches!(
      ret,
      Err(RuleSerializeError::MatchesReference(
        ReferentRuleError::CyclicRule(rule)
      )) if rule == "RECUR"
    ));
    Ok(())
  }

  #[test]
  fn test_parameter_name_does_not_create_false_cycle() -> Result<()> {
    let utils = from_str(
      r"
loop(loop):
  matches: loop
",
    )
    .expect("failed to parse utils");
    let env = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils)?;
    let rule = from_str(
      r"
matches:
  loop:
    loop:
      kind: number
",
    )
    .expect("should parse rule");
    assert!(env.deserialize_rule(rule).is_ok());
    Ok(())
  }

  #[test]
  fn test_invalid_parameterized_utility_signature() {
    let utils = from_str(
      r"
wrap():
  kind: number
",
    )
    .expect("failed to parse utils");
    let ret = DeserializeEnv::new(TypeScript::Tsx).with_utils(&utils);
    assert!(matches!(
      ret,
      Err(RuleSerializeError::InvalidUtils(
        rule::ParameterizedUtilError::InvalidUtilitySignature(signature)
      )) if signature == "wrap()"
    ));
  }
}
