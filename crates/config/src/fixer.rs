use crate::rule::Relation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A pattern string or fix object to auto fix the issue.
/// It can reference metavariables appeared in rule.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum Fixer {
  Str(String),
  Config(FixConfig),
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FixConfig {
  template: String,
  expand_forward: Relation,
  // TODO: add these
  // expand_backward: RelationalRule,
  // prepend: String,
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::maybe::Maybe;

  #[test]
  fn test_parse() {
    let fixer: Fixer = from_str("test").expect("should parse");
    assert!(matches!(fixer, Fixer::Str(_)));
  }

  #[test]
  fn test_parse_object() -> Result<(), serde_yaml::Error> {
    let src = "{template: 'abc', expandForward: {regex: ',', stopBy: neighbor}}";
    let Fixer::Config(cfg) = from_str(src)? else {
      panic!("wrong parsing")
    };
    assert_eq!(cfg.template, "abc");
    let rule = cfg.expand_forward.rule;
    assert_eq!(rule.regex, Maybe::Present(",".to_string()));
    assert!(rule.pattern.is_absent());
    Ok(())
  }
}
