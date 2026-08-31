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
  min_severity: Severity,
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
  pub fn new_for_verify(filter: Option<&Regex>, include_off: bool) -> Self {
    Self {
      default_severity: if include_off {
        Some(Severity::Hint)
      } else {
        None
      },
      by_rule_id: HashMap::new(),
      rule_filter: filter.cloned(),
      min_severity: Severity::Off,
    }
  }
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
      min_severity: cli.min_severity.clone(),
    })
  }

  pub fn process_configs(
    &self,
    configs: Vec<RuleConfig<SgLang>>,
  ) -> Result<Vec<RuleConfig<SgLang>>> {
    let configs = if let Some(filter) = &self.rule_filter {
      filter_rule_by_regex(configs, filter)?
    } else {
      configs
    };
    let configs = configs
      .into_iter()
      .filter_map(|config| self.process_one_config(config))
      .collect();
    Ok(configs)
  }

  fn process_one_config(&self, mut config: RuleConfig<SgLang>) -> Option<RuleConfig<SgLang>> {
    // overwrite severity
    let overwrite = self.find(&config.id);
    overwrite.overwrite(&mut config);
    // remove rules that are below min_severity
    if config.severity >= self.min_severity {
      Some(config)
    } else {
      None
    }
  }

  pub fn find(&self, id: &str) -> OverwriteResult {
    let severity = self
      .by_rule_id
      .get(id)
      .cloned()
      .or_else(|| self.default_severity.clone());
    OverwriteResult { severity }
  }

  pub fn apply_min_severity(&self, severity: Severity) -> Severity {
    if severity >= self.min_severity {
      severity
    } else {
      Severity::Off
    }
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
