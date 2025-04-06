use crate::fixer::Fixer;
use crate::rule::referent_rule::RuleRegistration;
use crate::rule::Rule;
use crate::rule_config::RuleConfigError;
use crate::rule_core::RuleCoreError;
use crate::transform::{TransformError, Transformation};
use crate::RuleCore;

use ast_grep_core::language::Language;

use std::collections::{HashMap, HashSet};

type RResult<T> = std::result::Result<T, RuleCoreError>;

pub enum CheckHint<'r> {
  Global,
  Normal,
  Rewriter(&'r HashSet<&'r str>),
}

/// Different rule sections have different variable scopes/check procedure.
/// so we need to check rules with different hints.
pub fn check_rule_with_hint<'r, L: Language>(
  rule: &'r Rule<L>,
  utils: &'r RuleRegistration<L>,
  constraints: &'r HashMap<String, Rule<L>>,
  transform: &'r Option<HashMap<String, Transformation>>,
  fixer: &Option<Fixer<L>>,
  hint: CheckHint<'r>,
) -> RResult<()> {
  match hint {
    CheckHint::Global => {
      // do not check utils defined here because global rules are not yet ready
      check_vars(rule, utils, constraints, transform, fixer)?;
    }
    CheckHint::Normal => {
      check_utils_defined(rule, constraints)?;
      check_vars(rule, utils, constraints, transform, fixer)?;
    }
    // upper_vars is needed to check metavar defined in containing vars
    CheckHint::Rewriter(upper_vars) => {
      check_utils_defined(rule, constraints)?;
      check_vars_in_rewriter(rule, utils, constraints, transform, fixer, upper_vars)?;
    }
  }
  Ok(())
}

fn check_vars_in_rewriter<'r, L: Language>(
  rule: &'r Rule<L>,
  utils: &'r RuleRegistration<L>,
  constraints: &'r HashMap<String, Rule<L>>,
  transform: &'r Option<HashMap<String, Transformation>>,
  fixer: &Option<Fixer<L>>,
  upper_var: &HashSet<&str>,
) -> RResult<()> {
  let vars = get_vars_from_rules(rule, utils);
  let vars = check_var_in_constraints(vars, constraints)?;
  let mut vars = check_var_in_transform(vars, transform)?;
  for v in upper_var {
    vars.insert(v);
  }
  check_var_in_fix(vars, fixer)?;
  Ok(())
}

fn check_utils_defined<L: Language>(
  rule: &Rule<L>,
  constraints: &HashMap<String, Rule<L>>,
) -> RResult<()> {
  rule.verify_util()?;
  for constraint in constraints.values() {
    constraint.verify_util()?;
  }
  Ok(())
}

fn check_vars<'r, L: Language>(
  rule: &'r Rule<L>,
  utils: &'r RuleRegistration<L>,
  constraints: &'r HashMap<String, Rule<L>>,
  transform: &'r Option<HashMap<String, Transformation>>,
  fixer: &Option<Fixer<L>>,
) -> RResult<()> {
  let vars = get_vars_from_rules(rule, utils);
  let vars = check_var_in_constraints(vars, constraints)?;
  let vars = check_var_in_transform(vars, transform)?;
  check_var_in_fix(vars, fixer)?;
  Ok(())
}

fn get_vars_from_rules<'r, L: Language>(
  rule: &'r Rule<L>,
  utils: &'r RuleRegistration<L>,
) -> HashSet<&'r str> {
  let mut vars = rule.defined_vars();
  for var in utils.get_local_util_vars() {
    vars.insert(var);
  }
  vars
}

fn check_var_in_constraints<'r, L: Language>(
  mut vars: HashSet<&'r str>,
  constraints: &'r HashMap<String, Rule<L>>,
) -> RResult<HashSet<&'r str>> {
  for rule in constraints.values() {
    for var in rule.defined_vars() {
      vars.insert(var);
    }
  }
  for var in constraints.keys() {
    let var: &str = var;
    if !vars.contains(var) {
      return Err(RuleCoreError::UndefinedMetaVar(
        var.to_owned(),
        "constraints",
      ));
    }
  }
  Ok(vars)
}

fn check_var_in_transform<'r>(
  mut vars: HashSet<&'r str>,
  transform: &'r Option<HashMap<String, Transformation>>,
) -> RResult<HashSet<&'r str>> {
  let Some(transform) = transform else {
    return Ok(vars);
  };
  for var in transform.keys() {
    // vars already has the transform value. Report error!
    if !vars.insert(var) {
      return Err(RuleCoreError::Transform(TransformError::AlreadyDefined(
        var.to_string(),
      )));
    }
  }
  for trans in transform.values() {
    let needed = trans.used_vars();
    if !vars.contains(needed) {
      return Err(RuleCoreError::UndefinedMetaVar(
        needed.to_string(),
        "transform",
      ));
    }
  }
  Ok(vars)
}

fn check_var_in_fix<L: Language>(vars: HashSet<&str>, fixer: &Option<Fixer<L>>) -> RResult<()> {
  let Some(fixer) = fixer else {
    return Ok(());
  };
  for var in fixer.used_vars() {
    if !vars.contains(&var) {
      return Err(RuleCoreError::UndefinedMetaVar(var.to_string(), "fix"));
    }
  }
  Ok(())
}

pub fn check_rewriters_in_transform<L: Language>(
  rule: &RuleCore<L>,
  rewriters: &HashMap<String, RuleCore<L>>,
) -> Result<(), RuleConfigError> {
  if let Some(err) = check_one_rewriter_in_rule(rule, rewriters) {
    return Err(err);
  }
  let error = rewriters
    .values()
    .find_map(|rewriter| check_one_rewriter_in_rule(rewriter, rewriters));
  if let Some(err) = error {
    return Err(err);
  }
  Ok(())
}

fn check_one_rewriter_in_rule<L: Language>(
  rule: &RuleCore<L>,
  rewriters: &HashMap<String, RuleCore<L>>,
) -> Option<RuleConfigError> {
  let transform = rule.transform.as_ref()?;
  let mut used_rewriters = transform
    .values()
    .flat_map(|trans| trans.used_rewriters().iter());
  let undefined_writers = used_rewriters.find(|r| !rewriters.contains_key(*r))?;
  Some(RuleConfigError::UndefinedRewriter(
    undefined_writers.to_string(),
  ))
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript;
  use crate::{from_str, DeserializeEnv, SerializableRuleCore};

  #[test]
  fn test_defined_vars() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore = from_str(
      r"
rule: {pattern: $A = $B}
constraints:
  A: { pattern: $C = $D }
transform:
  E:
    substring:
      source: $B
      startCar: 1",
    )
    .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    assert_eq!(
      matcher.defined_vars(),
      ["A", "B", "C", "D", "E"].into_iter().collect()
    );
  }

  fn get_undefined(src: &str) -> (String, &str) {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore = from_str(src).expect("should deser");
    match ser_rule.get_matcher(env) {
      Err(RuleCoreError::UndefinedMetaVar(name, section)) => (name, section),
      _ => panic!("unexpected error"),
    }
  }

  #[test]
  fn test_undefined_vars_in_constraints() {
    let (name, section) = get_undefined(
      r"
rule: {pattern: $A}
constraints: {B: {pattern: bbb}}
",
    );
    assert_eq!(name, "B");
    assert_eq!(section, "constraints");
  }
  #[test]
  fn test_undefined_vars_in_transform() {
    let (name, section) = get_undefined(
      r"
rule: {pattern: $A}
constraints: {A: {pattern: $C}}
transform:
  B:
    replace: {source: $C, replace: a, by: b }
  D:
    replace: {source: $E, replace: a, by: b }
",
    );
    assert_eq!(name, "E");
    assert_eq!(section, "transform");
  }
  #[test]
  fn test_undefined_vars_in_fix() {
    let (name, section) = get_undefined(
      r"
rule: {pattern: $A}
constraints: {A: {pattern: $C}}
transform:
  B:
    replace: {source: $C, replace: a, by: b }
fix: $D
",
    );
    assert_eq!(name, "D");
    assert_eq!(section, "fix");
  }

  #[test]
  fn test_defined_vars_in_utils() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore = from_str(
      r"
rule: {matches: test}
utils:
  test: { pattern: $B}",
    )
    .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    assert_eq!(matcher.defined_vars(), ["B"].into_iter().collect());
  }

  #[test]
  fn test_use_vars_in_utils() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore = from_str(
      r"
utils:
  test: { pattern: $B }
rule: { matches: test }
fix: $B = 123",
    )
    .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    assert_eq!(matcher.defined_vars(), ["B"].into_iter().collect());
  }

  #[test]
  fn test_defined_vars_cyclic() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore = from_str(
      r"
rule: { matches: test1 }
utils:
  test1: { pattern: $B, inside: {matches: test2} }
  test2: { pattern: $A, has: {matches: test1} }",
    )
    .expect("should deser");
    let matcher = ser_rule.get_matcher(env).expect("should parse");
    assert_eq!(matcher.defined_vars(), ["A", "B"].into_iter().collect());
  }

  #[test]
  fn test_transform_already_defined() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ser_rule: SerializableRuleCore = from_str(
      r"
rule: { pattern: $A = $B }
transform:
  B: { substring: { source: $A } }",
    )
    .expect("should deser");
    let matcher = ser_rule.get_matcher(env);
    match matcher {
      Err(RuleCoreError::Transform(TransformError::AlreadyDefined(b))) => {
        assert_eq!(b, "B");
      }
      _ => panic!("unexpected error"),
    }
  }
}
