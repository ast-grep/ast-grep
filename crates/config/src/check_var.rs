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
