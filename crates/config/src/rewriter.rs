use ast_grep_core::language::Language;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

use crate::check_var::{CheckHint, check_rewriters};
use crate::fixer::{Fixer, FixerError, SerializableFixer};
use crate::rule::DeserializeEnv;
use crate::{RuleCore, RuleCoreError, SerializableRuleCore};

pub struct Rewriter {
  pub matcher: RuleCore,
  pub fixer: Vec<Fixer>,
}
#[derive(Debug, Error)]
#[error("Rewriter `{id}` has invalid configuration.")]
pub struct RewriterError {
  pub id: String,
  #[source]
  pub reason: RewriterErrorReason,
}

#[derive(Debug, Error)]
pub enum RewriterErrorReason {
  #[error(transparent)]
  Core(#[from] RuleCoreError),
  #[error("Rewriter rule must have `fix`.")]
  NoFixInRewriter,
  #[error("`fix` pattern is invalid.")]
  Fixer(#[from] FixerError),
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
  ) -> Result<Rewriter, RewriterError> {
    let attach_id = |e| RewriterError {
      id: self.id.clone(),
      reason: e,
    };
    let rewriter = self
      .core
      .get_matcher_with_hint(env.clone(), CheckHint::Skip)
      .map_err(|e| attach_id(e.into()))?;
    let fixer =
      Fixer::parse(&self.fix, env, &self.core.transform).map_err(|e| attach_id(e.into()))?;
    if fixer.is_empty() {
      return Err(attach_id(RewriterErrorReason::NoFixInRewriter));
    }
    check_rewriters(&rewriter, &fixer, upper_vars).map_err(|e| attach_id(e.into()))?;
    Ok(Rewriter {
      matcher: rewriter,
      fixer,
    })
  }
}
