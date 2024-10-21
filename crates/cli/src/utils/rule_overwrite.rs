use super::SeverityArg;

use anyhow::Result;
use ast_grep_config::{SerializableRuleConfig, Severity};
use ast_grep_core::Language;

use std::collections::HashMap;

struct RuleOverwrite {
  default_severity: Option<Severity>,
  by_rule_id: HashMap<String, Severity>,
}

fn read_severity(
  severity: Severity,
  ids: &Option<Vec<String>>,
  by_rule_id: &mut HashMap<String, Severity>,
  default_severity: &mut Option<Severity>,
) {
  let Some(ids) = ids.as_ref() else { return };
  if ids.is_empty() {
    *default_severity = Some(severity);
    return;
  }
  for id in ids {
    by_rule_id.insert(id.clone(), severity.clone());
  }
}

impl RuleOverwrite {
  pub fn new(cli: SeverityArg) -> Result<Self> {
    let mut default_severity = None;
    let mut by_rule_id = HashMap::new();
    read_severity(
      Severity::Error,
      &cli.error,
      &mut by_rule_id,
      &mut default_severity,
    );
    Ok(Self {
      default_severity,
      by_rule_id,
    })
  }

  pub fn find(&self, id: &str) -> OverwriteResult {
    let severity = self
      .by_rule_id
      .get(id)
      .cloned()
      .or_else(|| self.default_severity.clone());
    OverwriteResult { severity }
  }
}

pub struct OverwriteResult {
  pub severity: Option<Severity>,
}

impl OverwriteResult {
  pub fn overwrite<L>(&self, rule: &mut SerializableRuleConfig<L>)
  where
    L: Language,
  {
    if let Some(severity) = &self.severity {
      rule.severity = severity.clone();
    }
  }
}
