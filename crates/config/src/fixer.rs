use crate::maybe::Maybe;
use crate::rule::{Relation, Rule, StopBy};
use ast_grep_core::replacer::IndentSensitive;
use ast_grep_core::replacer::{TemplateFix, TemplateFixError};
use ast_grep_core::Language;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A pattern string or fix object to auto fix the issue.
/// It can reference metavariables appeared in rule.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum SerializableFixer {
  Str(String),
  Config(SerializableFixConfig),
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerializableFixConfig {
  template: String,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  expand_end: Maybe<Relation>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  expand_start: Maybe<Relation>,
  // TODO: add these
  // prepend: String,
}

struct Expander<L: Language> {
  matches: Rule<L>,
  stop_by: StopBy<L>,
}

pub struct Fixer<C: IndentSensitive, L: Language> {
  template: TemplateFix<C>,
  expand_start: Option<Expander<L>>,
  expand_end: Option<Expander<L>>,
}

impl<C, L> Fixer<C, L>
where
  C: IndentSensitive,
  L: Language,
{
  pub fn parse(serialized: SerializableFixConfig, lang: &L) -> Result<Self, TemplateFixError> {
    todo!()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::maybe::Maybe;

  #[test]
  fn test_parse() {
    let fixer: SerializableFixer = from_str("test").expect("should parse");
    assert!(matches!(fixer, SerializableFixer::Str(_)));
  }

  #[test]
  fn test_parse_object() -> Result<(), serde_yaml::Error> {
    let src = "{template: 'abc', expandEnd: {regex: ',', stopBy: neighbor}}";
    let SerializableFixer::Config(cfg) = from_str(src)? else {
      panic!("wrong parsing")
    };
    assert_eq!(cfg.template, "abc");
    let rule = cfg.expand_end.unwrap().rule;
    assert_eq!(rule.regex, Maybe::Present(",".to_string()));
    assert!(rule.pattern.is_absent());
    Ok(())
  }
}
