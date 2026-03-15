//! Parameterized utility rules have two parts:
//!
//! 1. Definition: declare a utility template and its parameters, for example
//!    `wrap(BODY): ...`.
//! 2. Call: reference that template from `matches` and provide concrete rules
//!    for each parameter, for example `matches: { wrap: { BODY: ... } }`.
//!
//! This module contains both halves of that flow: parsing and validating
//! parameterized utility definitions, and lowering/executing parameterized
//! utility calls.

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

pub(crate) type LocalTemplate = Def<Rule>;
pub(crate) type GlobalTemplate = Def<RuleCore>;

#[derive(Debug, Error)]
pub enum ParseUtilError {
  #[error("Utility declaration `{0}` has an invalid signature.")]
  InvalidUtilitySignature(String),
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

  fn into_items(self) -> Result<SerializableUtilityItems, ParseUtilError> {
    if self.0.is_empty() {
      return Err(ParseUtilError::InvalidUtilityCall);
    }
    Ok(self.0.into_iter().collect())
  }
}

pub(super) struct UtilitySignature {
  pub(super) name: String,
  pub(super) params: Vec<String>,
}

impl UtilitySignature {
  pub(super) fn parse(raw: &str) -> Result<Self, ParseUtilError> {
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
  if env.has_current_param(&callee) {
    return Err(ParseUtilError::UtilityParameterCalled(callee).into());
  }
  let template_params = env.get_template_params(&callee).ok_or_else(|| {
    if env.has_declared_util(&callee) {
      ParseUtilError::UnexpectedUtilityArguments(callee.clone()).into()
    } else {
      RuleSerializeError::MatchesReference(ReferentRuleError::UndefinedUtil(callee.clone()))
    }
  })?;
  validate_utility_args(&callee, template_params, &args)?;
  let lowered_args = lower_utility_args(args, env)?;
  let matches = ReferentRule::new(callee.clone(), lowered_args, &env.registration);
  if matches
    .args
    .values()
    .any(|arg| arg.check_cyclic_with_params(&callee, env.current_params()))
  {
    return Err(ReferentRuleError::CyclicRule(callee).into());
  }
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
        .get_local_templates()
        .get(rule_id)
        .map(|template| template.matcher.verify_util())
        .or_else(|| reg_ref.get_global_templates().get(rule_id).map(|_| Ok(())))
        .unwrap_or_else(|| {
          if reg_ref.get_local().contains_key(rule_id) || reg_ref.get_global().contains_key(rule_id)
          {
            Err(ParseUtilError::UnexpectedUtilityArguments(rule_id.to_string()).into())
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
    .get_local_templates()
    .get(rule_id)
    .map(|template| template.matcher.potential_kinds())
    .or_else(|| {
      reg_ref
        .get_global_templates()
        .get(rule_id)
        .map(|template| template.matcher.potential_kinds())
    })
    .flatten()
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
    .get_local_templates()
    .get(rule_id)
    .map(|template| {
      with_arg_bindings(args.clone(), || {
        template.matcher.match_node_with_env(node.clone(), env)
      })
    })
    .or_else(|| {
      reg_ref
        .get_global_templates()
        .get(rule_id)
        .map(|template| match_global_template(template, args.clone(), exported_vars, node, env))
    })
    .flatten()
}

fn validate_utility_args(
  callee: &str,
  params: &[String],
  args: &HashMap<String, SerializableRule>,
) -> Result<(), ParseUtilError> {
  for name in args.keys() {
    if !params.iter().any(|param| param == name) {
      return Err(ParseUtilError::UnknownUtilityArgument {
        callee: callee.into(),
        arg: name.clone(),
      });
    }
  }
  for name in params {
    if !args.contains_key(name) {
      return Err(ParseUtilError::MissingUtilityArgument {
        callee: callee.into(),
        arg: name.clone(),
      });
    }
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
