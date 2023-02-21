use crate::constraints::RuleWithConstraint;
use crate::referent_rule::{Registration, RuleRegistration};
use crate::rule::{deserialize_rule, RuleSerializeError, SerializableRule};
use crate::rule_config::{RuleConfigError, SerializableRuleCore};

use ast_grep_core::language::Language;

use std::collections::HashMap;

pub struct DeserializeEnv<L: Language> {
  pub(crate) registration: RuleRegistration<L>,
  pub(crate) lang: L,
}

impl<L: Language> DeserializeEnv<L> {
  pub fn new(lang: L) -> Self {
    Self {
      registration: Default::default(),
      lang,
    }
  }

  pub fn register_local_utils(
    self,
    utils: &HashMap<String, SerializableRule>,
  ) -> Result<Self, RuleSerializeError> {
    for (id, rule) in utils {
      let rule = deserialize_rule(rule.clone(), &self)?;
      self.registration.insert_local(id, rule)?;
    }
    Ok(self)
  }

  pub fn parse_global_utils(
    utils: Vec<SerializableRuleCore<L>>,
  ) -> Result<Registration<RuleWithConstraint<L>>, RuleConfigError> {
    let registration = Registration::default();
    for rule in utils {
      let matcher = rule.get_matcher()?;
      registration
        .insert(&rule.id, matcher)
        .map_err(RuleSerializeError::MatchesRefrence)?;
    }
    Ok(registration)
  }

  pub fn with_globals(self, globals: &Registration<RuleWithConstraint<L>>) -> Self {
    Self {
      registration: RuleRegistration::from_globals(globals),
      lang: self.lang,
    }
  }
}
