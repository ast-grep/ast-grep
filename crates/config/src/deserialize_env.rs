use crate::referent_rule::{GlobalRules, RuleRegistration};
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
  ) -> Result<GlobalRules<L>, RuleConfigError> {
    let registration = GlobalRules::default();
    for rule in utils {
      let matcher = rule.get_matcher(&registration)?;
      registration
        .insert(&rule.id, matcher)
        .map_err(RuleSerializeError::MatchesRefrence)?;
    }
    Ok(registration)
  }

  pub fn with_globals(self, globals: &GlobalRules<L>) -> Self {
    Self {
      registration: RuleRegistration::from_globals(globals),
      lang: self.lang,
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;

  #[test]
  fn test_deserialize_local() {
    let utils = from_str(
      "
accessor-name:
  matches: member-name
  regex: whatever
member-name:
  kind: identifier
",
    )
    .unwrap();
    let env = DeserializeEnv::new(TypeScript::Tsx)
      .register_local_utils(&utils)
      .unwrap();
    let rule = from_str("matches: accessor-name").unwrap();
    let rule = deserialize_rule(rule, &env).unwrap();
    let grep = TypeScript::Tsx.ast_grep("whatever");
    assert!(grep.root().find(rule).is_some());
  }
}
