use crate::referent_rule::RuleRegistration;
use crate::rule::{deserialize_rule, RuleSerializeError, SerializableRule};

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

  pub fn register_utils(
    mut self,
    utils: &HashMap<String, SerializableRule>,
  ) -> Result<Self, RuleSerializeError> {
    let registration = RuleRegistration::default();
    for (id, rule) in utils {
      let rule = deserialize_rule(rule.clone(), &self)?;
      registration.insert_rule(id, rule)?;
    }
    self.registration = registration;
    Ok(self)
  }
}
