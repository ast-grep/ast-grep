//! Parameterized utility rules have two parts:
//!
//! 1. Definition: declare a utility template and its parameters, for example
//!    `id: wrap` with `arguments: [BODY]`.
//! 2. Call: reference that template from `matches` and provide concrete rules
//!    for each parameter, for example `matches: { wrap: { BODY: ... } }`.
//!
//! This module contains both halves of that flow: parsing and validating
//! parameterized utility definitions, and lowering/executing parameterized
//! utility calls.
//!
//! At the moment, parameterized definitions are only supported for global
//! utilities. Local `utils:` entries cannot declare parameters.

use super::deserialize_env::DeserializeEnv;
use super::referent_rule::{ReferentRule, ReferentRuleError, RegistrationRef};
use super::{deserialize_rule, Rule, RuleSerializeError, SerializableRule};
use crate::RuleCore;

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::{MetaVarEnv, MetaVariable};
use ast_grep_core::ops as o;
use ast_grep_core::{Doc, Matcher, Node};

use bit_set::BitSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

thread_local! {
  static VERIFY_STACK: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
  static ARG_RULE_FRAME: RefCell<Option<Arc<BindingFrame>>> = const { RefCell::new(None) };
  static ARG_RULE_EXPORT_ENV: RefCell<Vec<*mut ()>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone)]
struct BindingFrame {
  bindings: Arc<HashMap<String, Arc<Rule>>>,
  parent: Option<Arc<BindingFrame>>,
}

type SerializableUtilityArgs = HashMap<String, SerializableRule>;
type SerializableUtilityCalls = HashMap<String, SerializableUtilityArgs>;
type SerializableUtilityItems = Vec<(String, SerializableUtilityArgs)>;

pub(crate) struct Def<M> {
  pub params: Vec<String>,
  pub matcher: M,
}

impl<M> Def<M> {
  pub(crate) fn new(params: Vec<String>, matcher: M) -> Self {
    Self { params, matcher }
  }
}

pub(crate) type GlobalTemplate = Def<RuleCore>;

#[derive(Debug, Error)]
pub enum ParameterizedUtilError {
  #[error("Utility id `{0}` contains reserved characters.")]
  InvalidUtilityId(String),
  #[error("Utility `{util}` declares invalid argument `{arg}`.")]
  InvalidUtilityArgument { util: String, arg: String },
  #[error("Utility `{util}` declares duplicate argument `{arg}`.")]
  DuplicateUtilityArgument { util: String, arg: String },
  #[error("Utility call must contain at least one callee.")]
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
#[serde(transparent)]
pub struct SerializableUtilityCall(pub SerializableUtilityCalls);

impl SerializableUtilityCall {
  pub(super) fn iter(&self) -> impl Iterator<Item = (&String, &SerializableUtilityArgs)> {
    self.0.iter()
  }

  fn into_items(self) -> Result<SerializableUtilityItems, ParameterizedUtilError> {
    if self.0.is_empty() {
      return Err(ParameterizedUtilError::InvalidUtilityCall);
    }
    Ok(self.0.into_iter().collect())
  }
}

fn contains_reserved_utility_chars(raw: &str) -> bool {
  raw.contains(['(', ')', ',', '='])
}

pub(super) fn validate_utility_id(raw: &str) -> Result<(), ParameterizedUtilError> {
  if raw.is_empty() || contains_reserved_utility_chars(raw) {
    return Err(ParameterizedUtilError::InvalidUtilityId(raw.into()));
  }
  Ok(())
}

pub(super) fn validate_utility_arguments(
  util: &str,
  params: &[String],
) -> Result<(), ParameterizedUtilError> {
  let mut seen = HashSet::new();
  for param in params {
    if param.is_empty() || contains_reserved_utility_chars(param) {
      return Err(ParameterizedUtilError::InvalidUtilityArgument {
        util: util.into(),
        arg: param.clone(),
      });
    }
    if !seen.insert(param.as_str()) {
      return Err(ParameterizedUtilError::DuplicateUtilityArgument {
        util: util.into(),
        arg: param.clone(),
      });
    }
  }
  Ok(())
}

pub(super) fn deserialize_utility_call_matches<L: Language>(
  call: SerializableUtilityCall,
  env: &DeserializeEnv<L>,
) -> Result<Rule, RuleSerializeError> {
  let mut rules = Vec::new();
  for (callee, args) in call.into_items()? {
    rules.push(lower_utility_call(callee, args, env)?);
  }
  if rules.len() == 1 {
    Ok(rules.pop().expect("must contain one rule"))
  } else {
    Ok(Rule::All(o::All::new(rules)))
  }
}

fn lower_utility_call<L: Language>(
  callee: String,
  args: HashMap<String, SerializableRule>,
  env: &DeserializeEnv<L>,
) -> Result<Rule, RuleSerializeError> {
  if env.registration.has_current_param(&callee) {
    return Err(ParameterizedUtilError::UtilityParameterCalled(callee).into());
  }
  let template_params = env
    .registration
    .get_util_template_params(&callee)
    .ok_or_else(|| {
      if env.registration.has_util(&callee) {
        ParameterizedUtilError::UnexpectedUtilityArguments(callee.clone()).into()
      } else {
        RuleSerializeError::MatchesReference(ReferentRuleError::UndefinedUtil(callee.clone()))
      }
    })?;
  validate_utility_args(&callee, template_params, &args)?;
  let lowered_args = lower_utility_args(args, env)?;
  if lowered_args.values().any(|arg| arg.check_cyclic(&callee)) {
    return Err(ReferentRuleError::CyclicRule(callee).into());
  }
  let matches = ReferentRule::new(callee.clone(), lowered_args, &env.registration);
  Ok(Rule::Matches(matches))
}

pub(crate) fn verify_parameterized_referent(
  rule_id: &str,
  args: &Arc<HashMap<String, Arc<Rule>>>,
  reg_ref: &RegistrationRef,
) -> Result<(), RuleSerializeError> {
  let should_verify = VERIFY_STACK.with(|stack| {
    let mut stack = stack.borrow_mut();
    if stack.contains(rule_id) {
      false
    } else {
      stack.insert(rule_id.to_string());
      true
    }
  });
  if !should_verify {
    return Ok(());
  }
  let result = args
    .values()
    .try_for_each(|arg| arg.verify_util())
    .and_then(|_| {
      reg_ref
        .get_global_templates()
        .get(rule_id)
        .map(|_| Ok(()))
        .unwrap_or_else(|| {
          if reg_ref.get_local().contains_key(rule_id) || reg_ref.get_global().contains_key(rule_id)
          {
            Err(ParameterizedUtilError::UnexpectedUtilityArguments(rule_id.to_string()).into())
          } else {
            Err(RuleSerializeError::MatchesReference(
              ReferentRuleError::UndefinedUtil(rule_id.to_string()),
            ))
          }
        })
    });
  VERIFY_STACK.with(|stack| {
    stack.borrow_mut().remove(rule_id);
  });
  result
}

pub(crate) fn parameterized_potential_kinds(
  rule_id: &str,
  reg_ref: &RegistrationRef,
) -> Option<BitSet> {
  reg_ref
    .get_global_templates()
    .get(rule_id)
    .and_then(|template| template.matcher.potential_kinds())
}

pub(crate) fn match_parameterized_referent<'tree, D: Doc>(
  rule_id: &str,
  args: Arc<HashMap<String, Arc<Rule>>>,
  exported_vars: &HashSet<String>,
  reg_ref: &RegistrationRef,
  node: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  reg_ref
    .get_global_templates()
    .get(rule_id)
    .and_then(|template| match_global_template(template, args.clone(), exported_vars, node, env))
}

fn validate_utility_args(
  callee: &str,
  params: &[String],
  args: &HashMap<String, SerializableRule>,
) -> Result<(), ParameterizedUtilError> {
  for name in args.keys() {
    if !params.iter().any(|param| param == name) {
      return Err(ParameterizedUtilError::UnknownUtilityArgument {
        callee: callee.into(),
        arg: name.clone(),
      });
    }
  }
  // After verifying all arg keys are valid params, a length mismatch
  // means a declared param is missing from the call arguments.
  if args.len() < params.len() {
    let missing = params.iter().find(|p| !args.contains_key(p.as_str()));
    return Err(ParameterizedUtilError::MissingUtilityArgument {
      callee: callee.into(),
      arg: missing.unwrap().clone(),
    });
  }
  Ok(())
}

fn lower_utility_args<L: Language>(
  args: HashMap<String, SerializableRule>,
  env: &DeserializeEnv<L>,
) -> Result<HashMap<String, Rule>, RuleSerializeError> {
  let mut lowered = HashMap::with_capacity(args.len());
  for (name, rule) in args {
    lowered.insert(name, deserialize_rule(rule, env)?);
  }
  Ok(lowered)
}

pub(crate) fn with_arg_bindings<T>(
  bindings: Arc<HashMap<String, Arc<Rule>>>,
  f: impl FnOnce() -> T,
) -> T {
  let parent = ARG_RULE_FRAME.with(|current| current.borrow().clone());
  let frame = Arc::new(BindingFrame { bindings, parent });
  with_binding_frame(Some(frame), f)
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
      // Bound argument rules are intentionally isolated from the caller env.
      // They match against a temporary env seeded only from prior argument
      // exports in the current parameterized call. Export to the caller happens
      // later, after the whole template has matched, so export conflicts do not
      // trigger backtracking here.
      let mut local_env = Cow::Owned(export_env.clone());
      let matched = with_binding_frame(parent, || rule.match_node_with_env(node, &mut local_env))?;
      export_vars(local_env.as_ref(), export_env, &exported_vars)?;
      Some(matched)
    } else {
      with_binding_frame(parent, || rule.match_node_with_env(node, env))
    }
  })
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
  if !exported_vars.is_empty() {
    export_vars(&export_env, env.to_mut(), exported_vars)?;
  }
  Some(matched)
}

fn export_vars<'tree, D: Doc>(
  from: &MetaVarEnv<'tree, D>,
  to: &mut MetaVarEnv<'tree, D>,
  vars: &HashSet<String>,
) -> Option<()> {
  if vars.is_empty() {
    return Some(());
  }
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

fn lookup_bound_rule(name: &str) -> Option<(Arc<Rule>, Option<Arc<BindingFrame>>)> {
  ARG_RULE_FRAME.with(|current| {
    let borrow = current.borrow();
    let mut frame = borrow.as_ref();
    while let Some(active) = frame {
      if let Some(rule) = active.bindings.get(name) {
        return Some((rule.clone(), active.parent.clone()));
      }
      frame = active.parent.as_ref();
    }
    None
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

  #[test]
  fn test_validate_utility_id() {
    assert!(validate_utility_id("wrap").is_ok());
    assert!(matches!(
      validate_utility_id("wrap(BODY)"),
      Err(ParameterizedUtilError::InvalidUtilityId(id)) if id == "wrap(BODY)"
    ));
  }

  #[test]
  fn test_validate_utility_arguments() {
    assert!(validate_utility_arguments("wrap", &["BODY".into(), "EXPR".into()]).is_ok());
    assert!(matches!(
      validate_utility_arguments("wrap", &["BODY".into(), "BODY".into()]),
      Err(ParameterizedUtilError::DuplicateUtilityArgument { util, arg })
        if util == "wrap" && arg == "BODY"
    ));
    assert!(matches!(
      validate_utility_arguments("wrap", &["BODY(EXPR)".into()]),
      Err(ParameterizedUtilError::InvalidUtilityArgument { util, arg })
        if util == "wrap" && arg == "BODY(EXPR)"
    ));
  }
}
