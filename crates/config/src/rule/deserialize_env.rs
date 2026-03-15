use super::referent_rule::{GlobalRules, GlobalTemplate, ReferentRuleError, RuleRegistration};
use crate::check_var::CheckHint;
use crate::maybe::Maybe;
use crate::rule::{self, Rule, RuleSerializeError, SerializableMatches, SerializableRule};
use crate::rule_core::{RuleCoreError, SerializableRuleCore};
use crate::transform::Trans;
use ast_grep_core::meta_var::MetaVariable;

use ast_grep_core::language::Language;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Error)]
pub enum ParseUtilError {
  #[error("Utility declaration `{0}` has an invalid signature.")]
  InvalidUtilitySignature(String),
  #[error("Utility `{util}` declares duplicate argument `{arg}`.")]
  DuplicateUtilityArgument { util: String, arg: String },
  #[error("Utility call must contain exactly one callee.")]
  InvalidUtilityCall,
  #[error("Utility `{0}` requires arguments and cannot be used as `matches: {0}`.")]
  MissingUtilityArguments(String),
  #[error("Utility `{0}` does not accept arguments.")]
  UnexpectedUtilityArguments(String),
  #[error("Utility parameter `{0}` cannot be called with arguments.")]
  UtilityParameterCalled(String),
  #[error("Parameterized utility `{callee}` is missing argument `{arg}`.")]
  MissingUtilityArgument { callee: String, arg: String },
  #[error("Parameterized utility `{callee}` does not declare argument `{arg}`.")]
  UnknownUtilityArgument { callee: String, arg: String },
}

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
  /// utility signatures in the current local scope
  local_utils: HashMap<String, Vec<String>>,
  /// current parameter bindings allowed during deserialization
  current_params: Option<Arc<HashSet<String>>>,
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
        if !params.is_some_and(|params| params.iter().any(|param| param == id)) {
          sort.visit(id)?;
        }
      }
      SerializableMatches::Call(call) => {
        let Some((callee, args)) = call.0.iter().next() else {
          return Ok(());
        };
        if call.0.len() != 1 {
          return Ok(());
        }
        sort.visit(callee)?;
        for arg in args.values() {
          visit_dependent_rule_ids_with_params(arg, sort, params)?;
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
      local_utils: Default::default(),
      current_params: None,
    }
  }

  pub(crate) fn from_registration(lang: L, registration: RuleRegistration) -> Self {
    Self {
      registration,
      lang,
      local_utils: Default::default(),
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
    let env = self.with_local_utils(&parsed);
    for (id, util) in &parsed {
      if util.params.is_empty() {
        continue;
      }
      let params = util.params.iter().cloned().collect();
      let template = env
        .with_params(params)
        .deserialize_rule(util.body.clone())?;
      env
        .registration
        .insert_local_template(id, util.params.clone(), template)
        .map_err(RuleSerializeError::MatchesReference)?;
    }
    for id in order {
      let Some(util) = parsed.get(id) else {
        continue;
      };
      if !util.params.is_empty() {
        continue;
      }
      let rule = env.deserialize_rule(util.body.clone())?;
      env.registration.insert_local(id, rule)?;
    }
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
    for id in order {
      let parsed = utils.get(id).expect("must exist");
      let env = DeserializeEnv::new(parsed.lang.clone()).with_globals(&registration);
      if parsed.params.is_empty() {
        let matcher = parsed.core.get_matcher_with_hint(env, CheckHint::Global)?;
        registration
          .insert(id, matcher)
          .map_err(RuleSerializeError::MatchesReference)?;
      } else {
        let params = parsed.params.iter().cloned().collect();
        let matcher = parsed
          .core
          .get_matcher_with_hint(env.with_params(params), CheckHint::Global)?;
        registration
          .insert_template(id, GlobalTemplate::new(parsed.params.clone(), matcher))
          .map_err(RuleSerializeError::MatchesReference)?;
      }
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
      registration: RuleRegistration::from_globals(globals),
      lang: self.lang,
      local_utils: Default::default(),
      current_params: self.current_params,
    }
  }

  pub(crate) fn get_template_params(&self, id: &str) -> Option<&Vec<String>> {
    self
      .local_utils
      .get(id)
      .filter(|params| !params.is_empty())
      .or_else(|| self.registration.get_global_template_params(id))
  }

  pub(crate) fn has_declared_util(&self, id: &str) -> bool {
    self.local_utils.contains_key(id) || self.registration.has_global_rule(id)
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

  fn with_local_utils(mut self, utils: &ParsedUtils) -> Self {
    self.local_utils = utils
      .iter()
      .map(|(id, util)| (id.clone(), util.params.clone()))
      .collect();
    self
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

struct UtilitySignature {
  name: String,
  params: Vec<String>,
}

impl UtilitySignature {
  fn parse(raw: &str) -> Result<Self, ParseUtilError> {
    let Some(paren) = raw.find('(') else {
      if raw.contains(')') {
        return Err(ParseUtilError::InvalidUtilitySignature(raw.into()));
      }
      return Ok(Self {
        name: raw.into(),
        params: vec![],
      });
    };
    if !raw.ends_with(')') {
      return Err(ParseUtilError::InvalidUtilitySignature(raw.into()));
    }
    let name = raw[..paren].trim();
    let inner = &raw[paren + 1..raw.len() - 1];
    if name.is_empty() || inner.trim().is_empty() {
      return Err(ParseUtilError::InvalidUtilitySignature(raw.into()));
    }
    let mut params = Vec::new();
    let mut seen = HashSet::new();
    for param in inner.split(',').map(str::trim) {
      if param.is_empty() {
        return Err(ParseUtilError::InvalidUtilitySignature(raw.into()));
      }
      if !seen.insert(param.to_string()) {
        return Err(ParseUtilError::DuplicateUtilityArgument {
          util: name.into(),
          arg: param.into(),
        });
      }
      params.push(param.to_string());
    }
    Ok(Self {
      name: name.into(),
      params,
    })
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
        rule::ParseUtilError::MissingUtilityArgument {
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
        rule::ParseUtilError::UnknownUtilityArgument {
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
        rule::ParseUtilError::InvalidUtilitySignature(signature)
      )) if signature == "wrap()"
    ));
  }
}
