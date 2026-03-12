use crate::{Rule, RuleCore};

use ast_grep_core::meta_var::{MetaVarEnv, MetaVariable};
use ast_grep_core::{Doc, Matcher, Node};

use bit_set::BitSet;
use thiserror::Error;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

thread_local! {
  static VERIFY_STACK: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
  static VERIFY_PARAM_STACK: RefCell<Vec<Arc<HashSet<String>>>> = const { RefCell::new(Vec::new()) };
  static POTENTIAL_KINDS_STACK: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
  static POTENTIAL_PARAM_STACK: RefCell<Vec<Arc<HashSet<String>>>> = const { RefCell::new(Vec::new()) };
  static ARG_RULE_FRAME: RefCell<Option<Arc<BindingFrame>>> = const { RefCell::new(None) };
  static ARG_RULE_EXPORT_ENV: RefCell<Vec<*mut ()>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone)]
struct BindingFrame {
  bindings: Arc<HashMap<String, Arc<Rule>>>,
  parent: Option<Arc<BindingFrame>>,
}

fn with_potential_kinds_guard<T>(id: &str, compute: impl FnOnce() -> Option<T>) -> Option<T> {
  let should_compute = POTENTIAL_KINDS_STACK.with(|stack| {
    let mut stack = stack.borrow_mut();
    if stack.contains(id) {
      false
    } else {
      stack.insert(id.to_string());
      true
    }
  });
  if !should_compute {
    return None;
  }
  let ret = compute();
  POTENTIAL_KINDS_STACK.with(|stack| {
    stack.borrow_mut().remove(id);
  });
  ret
}

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

pub(crate) struct Def<M> {
  pub params: Vec<String>,
  pub matcher: M,
}

impl<M> Def<M> {
  pub(crate) fn new(params: Vec<String>, matcher: M) -> Self {
    Self { params, matcher }
  }
}

pub(crate) type LocalTemplate = Def<Rule>;
pub(crate) type GlobalTemplate = Def<RuleCore>;

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
  /// parameterized utility templates scoped to this rule config
  local_templates: Registration<LocalTemplate>,
  /// global rules are shared by all RuleConfigs. It is a singleton.
  global: Registration<RuleCore>,
  /// parameterized global rules are shared by all RuleConfigs. It is a singleton.
  global_templates: Registration<GlobalTemplate>,
  /// Every RuleConfig has its own rewriters. But sub-rules share parent's rewriters.
  rewriters: Registration<RuleCore>,
}

// these are shit code
impl RuleRegistration {
  pub fn get_rewriters(&self) -> &HashMap<String, RuleCore> {
    &self.rewriters.0
  }

  pub fn from_globals(global: &GlobalRules) -> Self {
    Self {
      local: Default::default(),
      local_templates: Default::default(),
      global: global.rules.clone(),
      global_templates: global.templates.clone(),
      rewriters: Default::default(),
    }
  }

  fn get_ref(&self) -> RegistrationRef {
    let local = Arc::downgrade(&self.local.0);
    let local_templates = Arc::downgrade(&self.local_templates.0);
    let global = Arc::downgrade(&self.global.0);
    let global_templates = Arc::downgrade(&self.global_templates.0);
    RegistrationRef {
      local,
      local_templates,
      global,
      global_templates,
    }
  }

  pub(crate) fn insert_local(&self, id: &str, rule: Rule) -> Result<(), ReferentRuleError> {
    let map = self.local.write();
    if map.contains_key(id) || self.local_templates.0.contains_key(id) {
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

  pub(crate) fn insert_local_template(
    &self,
    id: &str,
    params: Vec<String>,
    template: Rule,
  ) -> Result<(), ReferentRuleError> {
    let map = self.local_templates.write();
    if map.contains_key(id) || self.local.0.contains_key(id) {
      return Err(ReferentRuleError::DuplicateRule(id.into()));
    }
    map.insert(id.to_string(), LocalTemplate::new(params, template));
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
    for template in self.local_templates.0.values() {
      for v in template.matcher.defined_vars() {
        ret.insert(v);
      }
    }
    ret
  }

  pub(crate) fn has_global_rule(&self, id: &str) -> bool {
    self.global.0.contains_key(id) || self.global_templates.0.contains_key(id)
  }

  pub(crate) fn get_global_template_params(&self, id: &str) -> Option<&Vec<String>> {
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
  local_templates: Weak<HashMap<String, LocalTemplate>>,
  global: Weak<HashMap<String, RuleCore>>,
  global_templates: Weak<HashMap<String, GlobalTemplate>>,
}
impl RegistrationRef {
  fn get_local(&self) -> Arc<HashMap<String, Rule>> {
    self
      .local
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
  pub(crate) fn get_local_templates(&self) -> Arc<HashMap<String, LocalTemplate>> {
    self
      .local_templates
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
  fn get_global(&self) -> Arc<HashMap<String, RuleCore>> {
    self
      .global
      .upgrade()
      .expect("Rule Registration must be kept alive")
  }
  fn get_global_templates(&self) -> Arc<HashMap<String, GlobalTemplate>> {
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
pub struct ReferentRule {
  pub(crate) rule_id: String,
  pub(crate) args: Arc<HashMap<String, Arc<Rule>>>,
  /// Cached set of meta-variable names defined by the argument rules.
  exported_vars: HashSet<String>,
  reg_ref: RegistrationRef,
}

impl ReferentRule {
  pub fn try_new(
    rule_id: String,
    registration: &RuleRegistration,
  ) -> Result<Self, ReferentRuleError> {
    Ok(Self::new_with_ref(
      rule_id,
      HashMap::new(),
      registration.get_ref(),
    ))
  }

  pub fn new(
    rule_id: String,
    args: HashMap<String, Rule>,
    registration: &RuleRegistration,
  ) -> Self {
    Self::new_with_ref(rule_id, args, registration.get_ref())
  }

  pub(crate) fn new_with_ref(
    rule_id: String,
    args: HashMap<String, Rule>,
    reg_ref: RegistrationRef,
  ) -> Self {
    let args: HashMap<String, Arc<Rule>> = args
      .into_iter()
      .map(|(name, rule)| (name, Arc::new(rule)))
      .collect();
    let exported_vars = args
      .values()
      .flat_map(|rule| rule.defined_vars())
      .map(|s| s.to_string())
      .collect();
    Self {
      rule_id,
      args: Arc::new(args),
      exported_vars,
      reg_ref,
    }
  }

  fn is_parameterized(&self) -> bool {
    !self.args.is_empty()
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

  fn eval_global_template<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&GlobalTemplate) -> T,
  {
    let templates = self.reg_ref.get_global_templates();
    let template = templates.get(&self.rule_id)?;
    Some(func(template))
  }

  fn eval_local_template<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&Rule) -> T,
  {
    let templates = self.reg_ref.get_local_templates();
    let template = templates.get(&self.rule_id)?;
    Some(func(&template.matcher))
  }

  fn eval_local_template_params<F, T>(&self, func: F) -> Option<T>
  where
    F: FnOnce(&Vec<String>) -> T,
  {
    let templates = self.reg_ref.get_local_templates();
    let template = templates.get(&self.rule_id)?;
    Some(func(&template.params))
  }

  fn compute_potential_kinds(&self) -> Option<BitSet> {
    self
      .eval_local_template(|template| {
        with_arg_bindings(self.args.clone(), || template.potential_kinds())
      })
      .or_else(|| {
        self.eval_global_template(|template| {
          with_arg_bindings(self.args.clone(), || template.matcher.potential_kinds())
        })
      })
      .flatten()
  }

  pub(crate) fn defined_vars(&self) -> &HashSet<String> {
    &self.exported_vars
  }

  pub(super) fn verify_util(&self) -> Result<(), crate::rule::RuleSerializeError> {
    if !self.is_parameterized() {
      if has_verify_param(&self.rule_id) {
        return Ok(());
      }
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
        .get_local_templates()
        .contains_key(&self.rule_id)
        || self
          .reg_ref
          .get_global_templates()
          .contains_key(&self.rule_id)
      {
        return Err(crate::rule::RuleSerializeError::MissingUtilityArguments(
          self.rule_id.clone(),
        ));
      }
      return Err(crate::rule::RuleSerializeError::MatchesReference(
        ReferentRuleError::UndefinedUtil(self.rule_id.clone()),
      ));
    }
    let should_verify = VERIFY_STACK.with(|stack| {
      let mut stack = stack.borrow_mut();
      if stack.contains(&self.rule_id) {
        false
      } else {
        stack.insert(self.rule_id.clone());
        true
      }
    });
    if !should_verify {
      return Ok(());
    }
    let result = self
      .args
      .values()
      .try_for_each(|arg| arg.verify_util())
      .and_then(|_| {
        self
          .eval_local_template_params(|params| {
            let params = Arc::new(params.iter().cloned().collect::<HashSet<_>>());
            with_verify_params(params, || {
              self
                .eval_local_template(|template| template.verify_util())
                .expect("local template params and body must stay in sync")
            })
          })
          .or_else(|| self.eval_global_template(|_| Ok(())))
          .unwrap_or_else(|| {
            if self.reg_ref.get_local().contains_key(&self.rule_id)
              || self.reg_ref.get_global().contains_key(&self.rule_id)
            {
              Err(crate::rule::RuleSerializeError::UnexpectedUtilityArguments(
                self.rule_id.clone(),
              ))
            } else {
              Err(crate::rule::RuleSerializeError::MatchesReference(
                ReferentRuleError::UndefinedUtil(self.rule_id.clone()),
              ))
            }
          })
      });
    VERIFY_STACK.with(|stack| {
      stack.borrow_mut().remove(&self.rule_id);
    });
    result
  }

  pub(crate) fn check_cyclic_with_params(
    &self,
    id: &str,
    params: Option<&HashSet<String>>,
  ) -> bool {
    if self.is_parameterized() {
      self.rule_id == id
        || self
          .args
          .values()
          .any(|arg| arg.check_cyclic_with_params(id, params))
    } else {
      !params.is_some_and(|params| params.contains(&self.rule_id)) && self.rule_id == id
    }
  }
}

impl Matcher for ReferentRule {
  fn match_node_with_env<'tree, D: Doc>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if self.is_parameterized() {
      let exported_vars = self.defined_vars();
      self
        .eval_local_template(|template| {
          with_arg_bindings(self.args.clone(), || {
            template.match_node_with_env(node.clone(), env)
          })
        })
        .or_else(|| {
          self.eval_global_template(|template| {
            match_global_template(template, self.args.clone(), exported_vars, node, env)
          })
        })
        .flatten()
    } else {
      if lookup_bound_rule(&self.rule_id).is_some() {
        return match_bound_rule(&self.rule_id, node, env);
      }
      self
        .eval_local(|r| r.match_node_with_env(node.clone(), env))
        .or_else(|| self.eval_global(|r| match_global_rule(r, node, env)))
        .flatten()
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    if self.is_parameterized() {
      with_potential_kinds_guard(&self.rule_id, || self.compute_potential_kinds())
    } else {
      if lookup_bound_rule(&self.rule_id).is_some() || has_potential_param(&self.rule_id) {
        // Deliberately stop inferring kinds through parameter rule references.
        // A `matches: PARAM-RULE` edge is treated as "can match anything", both
        // during deserialization-time cache construction and during bound calls at
        // runtime. Users must provide stable kind information at the utility
        // definition site or around the call site if they want pruning to stay
        // precise and satisfy MissingPotentialKinds.
        return None;
      }
      self
        .eval_local(|r| {
          with_potential_kinds_guard(&self.rule_id, || {
            debug_assert!(!r.check_cyclic(&self.rule_id), "no cyclic rule allowed");
            r.potential_kinds()
          })
        })
        .or_else(|| {
          self.eval_global(|r| {
            with_potential_kinds_guard(&self.rule_id, || {
              debug_assert!(!r.check_cyclic(&self.rule_id), "no cyclic rule allowed");
              r.potential_kinds()
            })
          })
        })
        .flatten()
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

fn match_global_template<'tree, D: Doc>(
  template: &GlobalTemplate,
  bindings: Arc<HashMap<String, Arc<Rule>>>,
  exported_vars: &HashSet<String>,
  node: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  let mut local_env = Cow::Owned(MetaVarEnv::new());
  let mut export_env = MetaVarEnv::new();
  let matched = with_arg_export_env(&mut export_env, || {
    with_arg_bindings(bindings, || {
      template.matcher.match_node_with_env(node, &mut local_env)
    })
  })?;
  export_vars(&export_env, env.to_mut(), exported_vars)?;
  Some(matched)
}

fn export_vars<'tree, D: Doc>(
  from: &MetaVarEnv<'tree, D>,
  to: &mut MetaVarEnv<'tree, D>,
  vars: &HashSet<String>,
) -> Option<()> {
  for var in vars {
    if let Some(node) = from.get_match(var.as_str()) {
      to.insert(var, node.clone())?;
      continue;
    }
    let multi = from.get_multiple_matches(var.as_str());
    if !multi.is_empty() {
      to.insert_multi(var, multi)?;
      continue;
    }
    if let Some(bytes) = from.get_transformed(var.as_str()) {
      to.insert_transformation(
        &MetaVariable::Capture(var.to_string(), false),
        var,
        bytes.clone(),
      );
    }
  }
  Some(())
}

pub(crate) fn with_arg_bindings<T>(
  bindings: Arc<HashMap<String, Arc<Rule>>>,
  f: impl FnOnce() -> T,
) -> T {
  let parent = ARG_RULE_FRAME.with(|current| current.borrow().clone());
  let frame = Arc::new(BindingFrame { bindings, parent });
  with_binding_frame(Some(frame), f)
}

fn lookup_bound_rule(name: &str) -> Option<(Arc<Rule>, Option<Arc<BindingFrame>>)> {
  ARG_RULE_FRAME.with(|current| {
    let mut frame = current.borrow().clone();
    while let Some(active) = frame {
      if let Some(rule) = active.bindings.get(name) {
        return Some((rule.clone(), active.parent.clone()));
      }
      frame = active.parent.clone();
    }
    None
  })
}

pub(crate) fn match_bound_rule<'tree, D: Doc>(
  name: &str,
  node: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  let (rule, parent) = lookup_bound_rule(name)?;
  with_current_arg_export_env(|export_env: Option<&mut MetaVarEnv<'tree, D>>| {
    if let Some(export_env) = export_env {
      let exported_vars: HashSet<String> =
        rule.defined_vars().into_iter().map(String::from).collect();
      let mut local_env = Cow::Owned(export_env.clone());
      let matched = with_binding_frame(parent, || rule.match_node_with_env(node, &mut local_env))?;
      export_vars(local_env.as_ref(), export_env, &exported_vars)?;
      Some(matched)
    } else {
      with_binding_frame(parent, || rule.match_node_with_env(node, env))
    }
  })
}

fn with_binding_frame<T>(frame: Option<Arc<BindingFrame>>, f: impl FnOnce() -> T) -> T {
  struct FrameGuard(Option<Arc<BindingFrame>>);
  impl Drop for FrameGuard {
    fn drop(&mut self) {
      ARG_RULE_FRAME.with(|current| {
        *current.borrow_mut() = self.0.take();
      });
    }
  }
  let previous = ARG_RULE_FRAME.with(|current| current.replace(frame));
  let _guard = FrameGuard(previous);
  f()
}

macro_rules! define_param_stack_ops {
  ($with_fn:ident, $has_fn:ident, $stack:ident, $vis:vis) => {
    $vis fn $with_fn<T>(params: Arc<HashSet<String>>, f: impl FnOnce() -> T) -> T {
      struct Guard;
      impl Drop for Guard {
        fn drop(&mut self) {
          $stack.with(|stack| { stack.borrow_mut().pop(); });
        }
      }
      $stack.with(|stack| stack.borrow_mut().push(params));
      let _guard = Guard;
      f()
    }

    fn $has_fn(name: &str) -> bool {
      $stack.with(|stack| {
        stack.borrow().iter().rev().any(|params| params.contains(name))
      })
    }
  };
}

define_param_stack_ops!(with_verify_params, has_verify_param, VERIFY_PARAM_STACK, pub(crate));
define_param_stack_ops!(with_potential_params, has_potential_param, POTENTIAL_PARAM_STACK, pub(crate));

fn with_arg_export_env<'tree, D: Doc, T>(
  env: &mut MetaVarEnv<'tree, D>,
  f: impl FnOnce() -> T,
) -> T {
  struct ExportEnvGuard;
  impl Drop for ExportEnvGuard {
    fn drop(&mut self) {
      ARG_RULE_EXPORT_ENV.with(|stack| {
        stack.borrow_mut().pop();
      });
    }
  }
  ARG_RULE_EXPORT_ENV.with(|stack| {
    stack
      .borrow_mut()
      .push(env as *mut MetaVarEnv<'tree, D> as *mut ());
  });
  let _guard = ExportEnvGuard;
  f()
}

fn with_current_arg_export_env<'tree, D: Doc, T>(
  f: impl FnOnce(Option<&mut MetaVarEnv<'tree, D>>) -> T,
) -> T {
  let ptr = ARG_RULE_EXPORT_ENV.with(|stack| stack.borrow().last().copied());
  let env = ptr.map(|ptr| {
    // SAFETY: pointers are only pushed by `with_arg_export_env` for the duration
    // of the matching call on the same thread and with the same `D`/`'tree`.
    unsafe { &mut *(ptr as *mut MetaVarEnv<'tree, D>) }
  });
  f(env)
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
  use ast_grep_core::Language;
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
  fn test_template_conflicts_with_rule() {
    let registration = RuleRegistration::default();
    registration
      .insert_local_template("test", vec!["BODY".into()], Rule::default())
      .expect("template should insert");
    let rule = Rule::default();
    let error = registration.insert_local("test", rule);
    assert!(matches!(error, Err(ReferentRuleError::DuplicateRule(_))));
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

  #[test]
  fn test_parameterized_util_potential_kinds_are_conservative_for_all_and_any() -> Result {
    let registration = RuleRegistration::default();
    let arg_ref = Rule::Matches(ReferentRule::try_new("ARG".into(), &registration)?);

    registration.insert_local_template(
      "all-kind",
      vec!["ARG".into()],
      Rule::All(o::All::new([
        Rule::Kind(KindMatcher::new("number", TS::Tsx)),
        arg_ref,
      ])),
    )?;

    let all_number = ReferentRule::new(
      "all-kind".into(),
      HashMap::from([(
        "ARG".into(),
        Rule::Kind(KindMatcher::new("number", TS::Tsx)),
      )]),
      &registration,
    );
    let all_identifier = ReferentRule::new(
      "all-kind".into(),
      HashMap::from([(
        "ARG".into(),
        Rule::Kind(KindMatcher::new("identifier", TS::Tsx)),
      )]),
      &registration,
    );
    let number_id = usize::from(TS::Tsx.kind_to_id("number"));
    let identifier_id = usize::from(TS::Tsx.kind_to_id("identifier"));

    let all_number_kinds = all_number.potential_kinds().expect("all should be known");
    assert!(all_number_kinds.contains(number_id));
    assert!(!all_number_kinds.contains(identifier_id));

    let all_identifier_kinds = all_identifier
      .potential_kinds()
      .expect("all should be known");
    assert!(all_identifier_kinds.contains(number_id));
    assert!(!all_identifier_kinds.contains(identifier_id));

    registration.insert_local_template(
      "any-kind",
      vec!["ARG".into()],
      Rule::Any(o::Any::new([
        Rule::Kind(KindMatcher::new("number", TS::Tsx)),
        Rule::Matches(ReferentRule::try_new("ARG".into(), &registration)?),
      ])),
    )?;

    let any_number = ReferentRule::new(
      "any-kind".into(),
      HashMap::from([(
        "ARG".into(),
        Rule::Kind(KindMatcher::new("number", TS::Tsx)),
      )]),
      &registration,
    );
    let any_identifier = ReferentRule::new(
      "any-kind".into(),
      HashMap::from([(
        "ARG".into(),
        Rule::Kind(KindMatcher::new("identifier", TS::Tsx)),
      )]),
      &registration,
    );

    assert!(any_number.potential_kinds().is_none());

    assert!(any_identifier.potential_kinds().is_none());

    Ok(())
  }
}
