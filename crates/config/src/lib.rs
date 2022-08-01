pub mod support_language;
mod config_rule;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};

pub use support_language::SupportLang;
use config_rule::{SerializableRule, DynamicRule, from_serializable};


#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AstGrepRuleConfig {
    /// Unique, descriptive identifier, e.g., no-unused-variable
    pub id: String,
    /// Message highlighting why this rule fired and how to remediate the issue
    pub message: String,
    /// One of: Info, Warning, or Error
    pub severity: Severity,
    /// Specify the language to parse and the file extension to includ in matching.
    pub language: SupportLang,
    /// Pattern rules to find matching AST nodes
    pub rule: SerializableRule,
    /// Addtionaly meta variables pattern to filter matching
    #[serde(default)]
    pub meta_variables: HashMap<String, String>,
}

impl AstGrepRuleConfig {
    pub fn get_matcher(&self) -> DynamicRule<SupportLang> {
        from_serializable(self.rule.clone(), self.language)
    }
}


pub fn from_yaml_string(yaml: &str) -> Result<AstGrepRuleConfig, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}


#[cfg(test)]
mod test {

    use super::*;
    use ast_grep_core::language::Language;

    fn test_rule_match(yaml: &str, source: &str) {
        let config = from_yaml_string(yaml).expect("rule should parse");
        let grep = config.language.new(source);
        assert!(grep.root().find(config.get_matcher()).is_some());
    }

    fn test_rule_unmatch(yaml: &str, source: &str) {
        let config = from_yaml_string(yaml).expect("rule should parse");
        let grep = config.language.new(source);
        assert!(grep.root().find(config.get_matcher()).is_none());
    }

    fn make_yaml(rule: &str) -> String {
format!(r"
id: test
message: test rule
severity: info
language: TypeScript
rule:
{rule}
")
    }

    #[test]
    fn test_deserialize_rule_config() {
        let yaml = &make_yaml("
  pattern: let a = 123
");
        test_rule_match(yaml, "let a = 123; let b = 33;");
        test_rule_match(yaml, "class B { func() {let a = 123; }}");
        test_rule_unmatch(yaml, "const a = 33");
    }

    #[test]
    fn test_deserialize_nested() {
        let yaml = &make_yaml("
  all:
    - pattern: let $A = 123
    - pattern: let a = $B
");
        test_rule_match(yaml, "let a = 123; let b = 33;");
        test_rule_match(yaml, "class B { func() {let a = 123; }}");
        test_rule_unmatch(yaml, "const a = 33");
        test_rule_unmatch(yaml, "let a = 33");
    }

    #[test]
    fn test_deserialize_kind() {
        let yaml = &make_yaml("
    kind: class_body
");
        test_rule_match(yaml, "class B { func() {let a = 123; }}");
        test_rule_unmatch(yaml, "const B = { func() {let a = 123; }}");
    }

    #[test]
    fn test_deserialize_inside() {
        let yaml = &make_yaml("
  all:
    - inside:
        kind: class_body
    - pattern: let a = 123
");
        test_rule_unmatch(yaml, "let a = 123; let b = 33;");
        test_rule_match(yaml, "class B { func() {let a = 123; }}");
        test_rule_unmatch(yaml, "let a = 123");
    }

    #[test]
    fn test_deserialize_not_inside() {
        let yaml = &make_yaml("
  all:
    - not:
        inside:
          kind: class_body
    - pattern: let a = 123
");
        test_rule_match(yaml, "let a = 123; let b = 33;");
        test_rule_unmatch(yaml, "class B { func() {let a = 123; }}");
        test_rule_unmatch(yaml, "let a = 13");
    }

    #[test]
    fn test_deserialize_meta_var() {
        let yaml = &make_yaml("
  all:
    - inside:
        any:
          - pattern: function $A($$$) { $$$ }
          - pattern: let $A = ($$$) => $$$
    - pattern: $A($$$)
");
        test_rule_match(yaml, "function recursion() { recursion() }");
        test_rule_match(yaml, "let recursion = () => { recursion() }");
        test_rule_unmatch(yaml, "function callOther() { other() }");
    }
}
