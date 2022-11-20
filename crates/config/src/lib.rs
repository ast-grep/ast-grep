mod constraints;
mod rule;
mod rule_collection;

use serde::Deserialize;
use serde_yaml::{with::singleton_map_recursive::deserialize, Deserializer, Result};

use ast_grep_core::language::Language;

pub use rule::{
  try_deserialize_matchers, try_from_serializable as deserialize_rule, PatternStyle, Rule,
  RuleConfig, RuleWithConstraint, SerializableMetaVarMatcher, SerializableRule, Severity,
};
pub use rule_collection::RuleCollection;

pub fn from_str<'de, T: Deserialize<'de>>(s: &'de str) -> Result<T> {
  let deserializer = Deserializer::from_str(s);
  deserialize(deserializer)
}

pub fn from_yaml_string<'a, L: Language + Deserialize<'a>>(
  yamls: &'a str,
) -> Result<Vec<RuleConfig<L>>> {
  let mut ret = vec![];
  for yaml in Deserializer::from_str(yamls) {
    let config: RuleConfig<L> = deserialize(yaml)?;
    ret.push(config);
  }
  Ok(ret)
}
#[cfg(test)]
mod test {

  use super::*;
  use ast_grep_core::language::TSLanguage;
  use std::path::Path;
  #[derive(Clone, Deserialize, PartialEq, Eq)]
  pub enum TypeScript {
    Tsx,
  }
  impl Language for TypeScript {
    fn get_ts_language(&self) -> TSLanguage {
      tree_sitter_typescript::language_tsx().into()
    }
    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
      Some(TypeScript::Tsx)
    }
  }

  fn test_rule_match(yaml: &str, source: &str) {
    let config = &from_yaml_string::<TypeScript>(yaml).expect("rule should parse")[0];
    let grep = config.language.ast_grep(source);
    assert!(grep.root().find(config.get_matcher()).is_some());
  }

  fn test_rule_unmatch(yaml: &str, source: &str) {
    let config = &from_yaml_string::<TypeScript>(yaml).expect("rule should parse")[0];
    let grep = config.language.ast_grep(source);
    assert!(grep.root().find(config.get_matcher()).is_none());
  }

  fn make_yaml(rule: &str) -> String {
    format!(
      r"
id: test
message: test rule
severity: info
language: Tsx
rule:
{rule}
"
    )
  }

  #[test]
  fn test_deserialize_rule_config() {
    let yaml = &make_yaml(
      "
  pattern: let a = 123
",
    );
    test_rule_match(yaml, "let a = 123; let b = 33;");
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "const a = 33");
  }

  #[test]
  fn test_deserialize_nested() {
    let yaml = &make_yaml(
      "
  all:
    - pattern: let $A = 123
    - pattern: let a = $B
",
    );
    test_rule_match(yaml, "let a = 123; let b = 33;");
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "const a = 33");
    test_rule_unmatch(yaml, "let a = 33");
  }

  #[test]
  fn test_deserialize_kind() {
    let yaml = &make_yaml(
      "
    kind: class_body
",
    );
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "const B = { func() {let a = 123; }}");
  }

  #[test]
  fn test_deserialize_inside() {
    let yaml = &make_yaml(
      "
  all:
    - inside:
        kind: class_body
    - pattern: let a = 123
",
    );
    test_rule_unmatch(yaml, "let a = 123; let b = 33;");
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "let a = 123");
  }

  #[test]
  fn test_deserialize_not_inside() {
    let yaml = &make_yaml(
      "
  all:
    - not:
        inside:
          kind: class_body
    - pattern: let a = 123
",
    );
    test_rule_match(yaml, "let a = 123; let b = 33;");
    test_rule_unmatch(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "let a = 13");
  }

  #[test]
  fn test_deserialize_meta_var() {
    let yaml = &make_yaml(
      "
  all:
    - inside:
        any:
          - pattern: function $A($$$) { $$$ }
          - pattern: let $A = ($$$) => $$$
    - pattern: $A($$$)
",
    );
    test_rule_match(yaml, "function recursion() { recursion() }");
    test_rule_match(yaml, "let recursion = () => { recursion() }");
    test_rule_unmatch(yaml, "function callOther() { other() }");
  }
}
