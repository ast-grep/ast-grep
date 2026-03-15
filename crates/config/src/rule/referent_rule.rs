use super::parameterized_util::{
  match_bound_rule, match_parameterized_referent, parameterized_potential_kinds,
  verify_parameterized_referent, GlobalTemplate,
};
use crate::{Rule, RuleCore};

use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

pub struct Registration<R>(Arc<HashMap<String, R>>);

impl<R> Clone for Registration<R> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<R> Registration<R> {
  #[allow(clippy::mut_from_ref)]
  fn write(&self) -> &mut HashMap<String, R> {
    // SAFETY: `write` will only be called during initialization and
    // it only insert new item to the hashmap. It is safe to cast the raw ptr.
    unsafe { &mut *(Arc::as_ptr(&self.0) as *mut HashMap<String, R>) }
  }
}

impl<R> Default for Registration<R> {
  fn default() -> Self {
    Self(Default::default())
  }
}

#[derive(Clone, Default)]
pub struct GlobalRules {
  rules: Registration<RuleCore>,
  templates: Registration<GlobalTemplate>,
}

impl GlobalRules {
  pub fn insert(&self, id: &str, rule: RuleCore) -> Result<(), ReferentRuleError> {
    let map = self.rules.write();
    if map.contains_key(id) || self.templates.0.contains_key(id) {
      return Err(ReferentRuleError::DuplicateRule(id.into()));
    }
    map.insert(id.to_string(), rule);
    let rule = map.get(id).unwrap();
    // TODO: we can skip check here because insertion order
    // is guaranteed in deserialize_env
    if rule.check_cyclic(id) {
      return Err(ReferentRuleError::CyclicRule(id.to_string()));
    }
    Ok(())
  }

  pub(crate) fn insert_template(
    &self,
    id: &str,
    template: GlobalTemplate,
  ) -> Result<(), ReferentRuleError> {
    let map = self.templates.write();
    if map.contains_key(id) || self.rules.0.contains_key(id) {
      return Err(ReferentRuleError::DuplicateRule(id.into()));
    }
    map.insert(id.to_string(), template);
    Ok(())
  }
}

#[derive(Clone, Default)]
pub struct RuleRegistration {
  /// utility rule to every RuleCore, every sub-rule has its own local utility
  local: Registration<Rule>,
  /// global rules are shared by all RuleConfigs. It is a singleton.
  global: Registration<RuleCore>,
  /// parameterized global rules are shared by all RuleConfigs. It is a singleton.
  global_templates: Registration<GlobalTemplate>,
  /// Every RuleConfig has its own rewriters. But sub-rules share parent's rewriters.
  rewriters: Registration<RuleCore>,
  /// Current parameter bindings allowed while deserializing a global template.
  current_params: Option<Arc<HashSet<String>>>,
}

// these are shit code
impl RuleRegistration {
  pub fn get_rewriters(&self) -> &HashMap<String, RuleCore> {
    &self.rewriters.0
  }

  pub(crate) fn has_current_param(&self, id: &str) -> bool {
    self
      .current_params
      .as_deref()
      .is_some_and(|params| params.contains(id))
  }

  pub(crate) fn with_params(&self, params: HashSet<String>) -> Self {
    let mut registration = self.clone();
    registration.current_params = Some(Arc::new(params));
    registration
  }

  pub fn from_globals(global: &GlobalRules) -> Self {
    Self {
      local: Default::default(),
      global: global.rules.clone(),
      global_templates: global.templates.clone(),
      rewriters: Default::default(),
      current_params: None,
    }
  }

  fn get_ref(&self) -> RegistrationRef {
    let local = Arc::downgrade(&self.local.0);
    let global = Arc::downgrade(&self.global.0);
    let global_templates = Arc::downgrade(&self.global_templates.0);
    RegistrationRef {
      local,
      global,
      global_templates,
    }
  }

  pub(crate) fn insert_local(&self, id: &str, rule: Rule) -> Result<(), ReferentRuleError> {
    let map = self.local.write();
    if map.contains_key(id) {
      return Err(ReferentRuleError::DuplicateRule(id.into()));
    }
    map.insert(id.to_string(), rule);
    let rule = map.get(id).unwrap();
    // TODO: we can skip check here because insertion order
    // is guaranteed in deserialize_env
    if rule.check_cyclic(id) {
      return Err(ReferentRuleError::CyclicRule(id.to_string()));
    }
    Ok(())
  }

  pub(crate) fn insert_rewriter(&self, id: &str, rewriter: RuleCore) {
    let map = self.rewriters.write();
    map.insert(id.to_string(), rewriter);
  }

  pub(crate) fn get_local_util_vars(&self) -> HashSet<&str> {
    let mut ret = HashSet::new();
    for rule in self.local.0.values() {
      for v in rule.defined_vars() {
        ret.insert(v);
      }
    }
    ret
  }

  pub(crate) fn has_util(&self, id: &str) -> bool {
    self.local.0.contains_key(id)
      || self.global.0.contains_key(id)
      || self.global_templates.0.contains_key(id)
  }

  pub(crate) fn get_util_template_params(&self, id: &str) -> Option<&Vec<String>> {
    self
      .global_templates
      .0
      .get(id)
      .map(|template| &template.params)
  }
}

/// RegistrationRef must use Weak pointer to avoid
/// cyclic reference in RuleRegistration
#[derive(Clone)]
pub(crate) struct RegistrationRef {
  local: Weak<HashMap<String, Rule>>,
  global: Weak<HashMap<String, RuleCore>>,
  global_templates: Weak<HashMap<String, GlobalTemplate>>,
}
impl RegistrationRef {
  pub(crate) fn get_local(&self) -> Arc<HashMap<String, Rule>> {
    self
      .local
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
  pub(crate) fn get_global(&self) -> Arc<HashMap<String, RuleCore>> {
    self
      .global
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
  pub(crate) fn get_global_templates(&self) -> Arc<HashMap<String, GlobalTemplate>> {
    self
      .global_templates
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
}

#[derive(Debug, Error)]
pub enum ReferentRuleError {
  #[error("Rule `{0}` is not defined.")]
  UndefinedUtil(String),
  #[error("Duplicate rule id `{0}` is found.")]
  DuplicateRule(String),
  #[error("Rule `{0}` has a cyclic dependency in its `matches` sub-rule.")]
  CyclicRule(String),
}

#[derive(Clone)]
struct ReferentArgs {
  args: Arc<HashMap<String, Arc<Rule>>>,
  // a cache of variables, used in match
  exported_vars: HashSet<String>,
}

impl ReferentArgs {
  fn new(args: HashMap<String, Rule>) -> Self {
    let args: HashMap<String, Arc<Rule>> = args
      .into_iter()
      .map(|(name, rule)| (name, Arc::new(rule)))
      .collect();
    let exported_vars = args
      .values()
      .flat_map(|rule| rule.defined_vars())
      .map(|var| var.to_string())
      .collect();
    Self {
      args: Arc::new(args),
      exported_vars,
    }
  }
}

#[derive(Clone)]
enum ReferentFormat {
  Param,
  IdRef,
  // use Box to reduce the size
  Args(Box<ReferentArgs>),
}

#[derive(Clone)]
pub struct ReferentRule {
  pub(crate) rule_id: String,
  format: ReferentFormat,
  reg_ref: RegistrationRef,
}

impl ReferentRule {
  pub fn try_new(
    rule_id: String,
    registration: &RuleRegistration,
  ) -> Result<Self, ReferentRuleError> {
    Ok(Self {
      rule_id,
      format: ReferentFormat::IdRef,
      reg_ref: registration.get_ref(),
    })
  }

  pub(crate) fn try_new_param(
    rule_id: String,
    registration: &RuleRegistration,
  ) -> Result<Self, ReferentRuleError> {
    Ok(Self {
      rule_id,
      format: ReferentFormat::Param,
      reg_ref: registration.get_ref(),
    })
  }

  pub fn new(
    rule_id: String,
    args: HashMap<String, Rule>,
    registration: &RuleRegistration,
  ) -> Self {
    Self {
      rule_id,
      format: ReferentFormat::Args(Box::new(ReferentArgs::new(args))),
      reg_ref: registration.get_ref(),
    }
  }

  fn eval_local<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&Rule) -> T,
  {
    let rules = self.reg_ref.get_local();
    let rule = rules.get(&self.rule_id)?;
    Some(func(rule))
  }

  fn eval_global<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&RuleCore) -> T,
  {
    let rules = self.reg_ref.get_global();
    let rule = rules.get(&self.rule_id)?;
    Some(func(rule))
  }
  pub(crate) fn defined_vars(&self) -> HashSet<&str> {
    match &self.format {
      ReferentFormat::Args(args) => args.exported_vars.iter().map(String::as_str).collect(),
      ReferentFormat::Param | ReferentFormat::IdRef => HashSet::new(),
    }
  }

  pub(super) fn verify_util(&self) -> Result<(), crate::rule::RuleSerializeError> {
    match &self.format {
      ReferentFormat::Param => Ok(()),
      ReferentFormat::IdRef => {
        let rules = self.reg_ref.get_local();
        if rules.contains_key(&self.rule_id) {
          return Ok(());
        }
        let rules = self.reg_ref.get_global();
        if rules.contains_key(&self.rule_id) {
          return Ok(());
        }
        if self
          .reg_ref
          .get_global_templates()
          .contains_key(&self.rule_id)
        {
          return Err(
            crate::rule::ParameterizedUtilError::MissingUtilityArguments(self.rule_id.clone())
              .into(),
          );
        }
        Err(crate::rule::RuleSerializeError::MatchesReference(
          ReferentRuleError::UndefinedUtil(self.rule_id.clone()),
        ))
      }
      ReferentFormat::Args(args) => {
        verify_parameterized_referent(&self.rule_id, &args.args, &self.reg_ref)
      }
    }
  }

  pub(crate) fn check_cyclic(&self, id: &str) -> bool {
    match &self.format {
      ReferentFormat::Args(args) => {
        self.rule_id == id || args.args.values().any(|arg| arg.check_cyclic(id))
      }
      ReferentFormat::Param => false,
      ReferentFormat::IdRef => self.rule_id == id,
    }
  }
}

impl Matcher for ReferentRule {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    match &self.format {
      ReferentFormat::Args(args) => match_parameterized_referent(
        &self.rule_id,
        args.args.clone(),
        &args.exported_vars,
        &self.reg_ref,
        node,
        env,
      ),
      ReferentFormat::Param => match_bound_rule(&self.rule_id, node, env),
      ReferentFormat::IdRef => self
        .eval_local(|r| r.match_node_with_env(node.clone(), env))
        .or_else(|| self.eval_global(|r| match_global_rule(r, node, env)))
        .flatten(),
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    match &self.format {
      ReferentFormat::Args(_) => parameterized_potential_kinds(&self.rule_id, &self.reg_ref),
      ReferentFormat::Param => {
        // Deliberately stop inferring kinds through parameter rule references.
        // A `matches: PARAM-RULE` edge is treated as "can match anything", both
        // during deserialization-time cache construction and at runtime. Users
        // must provide stable kind information at the utility definition site
        // or around the call site if they want pruning to stay precise and
        // satisfy MissingPotentialKinds.
        None
      }
      ReferentFormat::IdRef => self
        .eval_local(|r| r.potential_kinds())
        .or_else(|| self.eval_global(|r| r.potential_kinds()))
        .flatten(),
    }
  }
}

fn match_global_rule<'tree, D: Doc>(
  rule: &RuleCore,
  node: Node<'tree, D>,
  _env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  let mut local_env = Cow::Owned(MetaVarEnv::new());
  let matched = rule.match_node_with_env(node, &mut local_env)?;
  Some(matched)
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::maybe::Maybe;
  use crate::rule::relational_rule::Relation;
  use crate::rule::stop_by::SerializableStopBy;
  use crate::rule::{Has, Rule, SerializableMatches, SerializableRule};
  use crate::test::TypeScript as TS;
  use ast_grep_core::matcher::KindMatcher;
  use ast_grep_core::ops as o;
  use ast_grep_core::Pattern;

  type Result = std::result::Result<(), ReferentRuleError>;

  #[test]
  fn test_cyclic_error() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::Matches(rule);
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule(_))));
    Ok(())
  }

  #[test]
  fn test_cyclic_all() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::All(o::All::new(std::iter::once(Rule::Matches(rule))));
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule(_))));
    Ok(())
  }

  #[test]
  fn test_cyclic_not() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let rule = Rule::Not(Box::new(o::Not::new(Rule::Matches(rule))));
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::CyclicRule(_))));
    Ok(())
  }

  #[test]
  fn test_success_rule() -> Result {
    let registration = RuleRegistration::default();
    let rule = ReferentRule::try_new("test".into(), &registration)?;
    let pattern = Rule::Pattern(Pattern::new("some", TS::Tsx));
    let ret = registration.insert_local("test", pattern);
    assert!(ret.is_ok());
    assert!(rule.potential_kinds().is_some());
    Ok(())
  }

  #[test]
  fn test_recursive_relation_potential_kinds_terminates() -> Result {
    let registration = RuleRegistration::default();
    let _recursive = ReferentRule::try_new("paren-number".into(), &registration)?;
    let env = crate::rule::DeserializeEnv::new(TS::Tsx);
    let number = Rule::Kind(KindMatcher::new("number", TS::Tsx));
    let paren = Rule::Kind(KindMatcher::new("parenthesized_expression", TS::Tsx));
    let nested = Rule::Has(Box::new(
      Has::try_new(
        Relation {
          rule: SerializableRule {
            matches: Maybe::Present(SerializableMatches::Id("paren-number".into())),
            ..Default::default()
          },
          stop_by: SerializableStopBy::End,
          field: None,
        },
        &env,
      )
      .expect("relation should deserialize"),
    ));
    let rule = Rule::Any(o::Any::new([
      number,
      Rule::All(o::All::new([paren, nested])),
    ]));
    registration.insert_local("paren-number", rule)?;
    let rule = ReferentRule::try_new("paren-number".into(), &registration)?;
    assert!(rule.potential_kinds().is_some());
    Ok(())
  }
}
