use ast_grep_core::language::Language;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::check_var::CheckHint;
use crate::rule::DeserializeEnv;
use crate::{RuleConfigError, RuleCore, SerializableRuleCore};

pub type Rewriter = RuleCore;

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRewriter {
  #[serde(flatten)]
  pub core: SerializableRuleCore,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
}

// TODO: change this to Rewriter::try_from
impl SerializableRewriter {
  pub fn try_parse_rewriter<L: Language>(
    &self,
    vars: &HashSet<&str>,
    env: &DeserializeEnv<L>,
  ) -> Result<Rewriter, RuleConfigError> {
    if self.core.fix.is_none() {
      return Err(RuleConfigError::NoFixInRewriter(self.id.clone()));
    }
    let rewriter = self
      .core
      .get_matcher_with_hint(env.clone(), CheckHint::Rewriter(vars))
      .map_err(|e| RuleConfigError::Rewriter(e, self.id.clone()))?;
    Ok(rewriter)
  }
}
