use crate::constraints::RuleWithConstraint;
use crate::referent_rule::RuleRegistration;
use crate::rule::{deserialize_rule, Rule, RuleSerializeError, SerializableRule};

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarMatchers;

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

  pub fn register_utils(
    mut self,
    utils: &HashMap<String, SerializableRule>,
  ) -> Result<Self, RuleSerializeError> {
    let registration = RuleRegistration::default();
    for (id, rule) in utils {
      let rule = RuleWithConstraint::new(
        deserialize_rule(rule.clone(), &self)?,
        MetaVarMatchers::default(),
      );
      registration.insert_rule(id, rule)?;
    }
    self.registration = registration;
    Ok(self)
  }
  pub fn with_registration(mut self, registration: RuleRegistration<L>) -> Self {
    self.registration = registration;
    self
  }
}
