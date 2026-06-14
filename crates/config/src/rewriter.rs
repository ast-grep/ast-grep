use ast_grep_core::language::Language;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::check_var::{CheckHint, check_rewriters};
use crate::fixer::{Fixer, SerializableFixer};
use crate::rule::DeserializeEnv;
use crate::{RuleConfigError, RuleCore, RuleCoreError, SerializableRuleCore};

pub struct Rewriter {
  pub matcher: RuleCore,
  pub fixer: Vec<Fixer>,
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRewriter {
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// A pattern string or a FixConfig object to auto fix the issue.
  /// It can reference metavariables appeared in rule.
  /// See details in fix [object reference](https://ast-grep.github.io/reference/yaml/fix.html#fixconfig).
  pub fix: SerializableFixer,
  #[serde(flatten)]
  pub core: SerializableRuleCore,
}

// TODO: change this to Rewriter::try_from
impl SerializableRewriter {
  pub fn try_parse_rewriter<L: Language>(
    &self,
    upper_vars: &HashSet<&str>,
    env: &DeserializeEnv<L>,
  ) -> Result<Rewriter, RuleConfigError> {
    // if self.core.fix.is_none() {
    //   return Err(RuleConfigError::NoFixInRewriter(self.id.clone()));
    // }
    let rewriter = self
      .core
      .get_matcher_with_hint(env.clone(), CheckHint::Skip)
      .map_err(|e| RuleConfigError::Rewriter(e, self.id.clone()))?;
    let fixer = Fixer::parse(&self.fix, env, &self.core.transform).map_err(RuleCoreError::Fixer)?;
    if fixer.is_empty() {
      return Err(RuleConfigError::NoFixInRewriter(self.id.clone()));
    }
    check_rewriters(
      &rewriter.rule,
      &rewriter.registration,
      &rewriter.constraints,
      &rewriter.transform,
      &fixer,
      upper_vars,
    )
    .map_err(|e| RuleConfigError::Rewriter(e, self.id.clone()))?;
    // TODO: add undefined var check here
    // see test_rewriter_fix_rejects_undefined_var
    Ok(Rewriter {
      matcher: rewriter,
      fixer,
    })
  }
}
