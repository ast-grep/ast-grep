use super::OverwriteArgs;
use crate::lang::SgLang;
use crate::utils::ErrorContext as EC;

use anyhow::Result;
use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::Language;
use regex::Regex;

use std::collections::HashMap;

#[derive(Default)]
pub struct RuleOverwrite {
  default_severity: Option<Severity>,
  by_rule_id: HashMap<String, Severity>,
  rule_filter: Option<Regex>,
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
  pub fn new(cli: &OverwriteArgs) -> Result<Self> {
    let mut default_severity = None;
    let mut by_rule_id = HashMap::new();
    read_severity(
      Severity::Error,
      &cli.error,
      &mut by_rule_id,
      &mut default_severity,
    );
    read_severity(
      Severity::Warning,
      &cli.warning,
      &mut by_rule_id,
      &mut default_severity,
    );
    read_severity(
      Severity::Info,
      &cli.info,
      &mut by_rule_id,
      &mut default_severity,
    );
    read_severity(
      Severity::Hint,
      &cli.hint,
      &mut by_rule_id,
      &mut default_severity,
    );
    read_severity(
      Severity::Off,
      &cli.off,
      &mut by_rule_id,
      &mut default_severity,
    );
    Ok(Self {
      default_severity,
      by_rule_id,
      rule_filter: cli.filter.clone(),
    })
  }

  pub fn process_configs(
    &self,
    configs: Vec<RuleConfig<SgLang>>,
  ) -> Result<Vec<RuleConfig<SgLang>>> {
    let mut configs = if let Some(filter) = &self.rule_filter {
      filter_rule_by_regex(configs, filter)?
    } else {
      configs
    };
    for config in &mut configs {
      let overwrite = self.find(&config.id);
      overwrite.overwrite(config);
    }
    Ok(configs)
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

fn filter_rule_by_regex(
  configs: Vec<RuleConfig<SgLang>>,
  filter: &Regex,
) -> Result<Vec<RuleConfig<SgLang>>> {
  let selected: Vec<_> = configs
    .into_iter()
    .filter(|c| filter.is_match(&c.id))
    .collect();

  if selected.is_empty() {
    Err(anyhow::anyhow!(EC::RuleNotFound(filter.to_string())))
  } else {
    Ok(selected)
  }
}

pub struct OverwriteResult {
  pub severity: Option<Severity>,
}

impl OverwriteResult {
  fn overwrite<L>(&self, rule: &mut RuleConfig<L>)
  where
    L: Language,
  {
    if let Some(severity) = &self.severity {
      rule.severity = severity.clone();
    }
  }
}
