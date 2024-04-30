use crate::fixer::Fixer;
use crate::rule::Rule;
use crate::rule_core::RuleConfigError;
use crate::transform::Transformation;

use ast_grep_core::language::Language;

use std::collections::{HashMap, HashSet};

type RResult<T> = std::result::Result<T, RuleConfigError>;

pub fn check_vars<'r, L: Language>(
  rule: &'r Rule<L>,
  constraints: &'r HashMap<String, Rule<L>>,
  transform: &'r Option<HashMap<String, Transformation>>,
  fixer: &Option<Fixer<L>>,
) -> RResult<()> {
  let vars = check_var_in_constraints(rule, constraints)?;
  let vars = check_var_in_transform(vars, transform)?;
  check_var_in_fix(vars, fixer)?;
  Ok(())
}

fn check_var_in_constraints<'r, L: Language>(
  rule: &'r Rule<L>,
  constraints: &'r HashMap<String, Rule<L>>,
) -> RResult<HashSet<&'r str>> {
  let mut vars = rule.defined_vars();
  for rule in constraints.values() {
    for var in rule.defined_vars() {
      vars.insert(var);
    }
  }
  for var in constraints.keys() {
    let var: &str = var;
    if !vars.contains(var) {
      return Err(RuleConfigError::UndefinedMetaVar(
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
    vars.insert(var);
  }
  for trans in transform.values() {
    let needed = trans.used_vars();
    if !vars.contains(needed) {
      return Err(RuleConfigError::UndefinedMetaVar(
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
      return Err(RuleConfigError::UndefinedMetaVar(var.to_string(), "fix"));
    }
  }
  Ok(())
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
      Err(RuleConfigError::UndefinedMetaVar(name, section)) => (name, section),
      _ => panic!("should error"),
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
}
