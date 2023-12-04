use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A pattern string or fix object to auto fix the issue.
/// It can reference metavariables appeared in rule.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum Fixer {
  Str(String),
  // Config(FixConfig),
}

// #[derive(Serialize, Deserialize, Clone, JsonSchema)]
// pub struct FixConfig {
//   template: String,
//   forward_expand: String,
//   backward_expand: String,
//   prepend: String,
// }

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;

  #[test]
  fn test_parse() {
    let fixer: Fixer = from_str("test").expect("should parse");
    assert!(matches!(fixer, Fixer::Str(_)));
  }
}
