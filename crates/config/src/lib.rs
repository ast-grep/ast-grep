pub mod support_language;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use ast_grep_core::{Rule, Matcher, PositiveMatcher, meta_var::MetaVarEnv};
use ast_grep_core as core;

pub use support_language::SupportLang;


#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SerializableRule {
    All(Vec<SerializableRule>),
    Any(Vec<SerializableRule>),
    Not(Box<SerializableRule>),
    Inside(Box<SerializableRule>),
    Has(Box<SerializableRule>),
    Pattern(String),
    Kind(String),
}

impl Matcher<SupportLang> for AstGrepRuleConfig {
    fn match_node_with_env<'tree>(&self, node: core::Node<'tree, SupportLang>, env: &mut MetaVarEnv<'tree, SupportLang>) -> Option<core::Node<'tree, SupportLang>> {
        use SerializableRule::*;
        let lang = self.language;
        match &self.rule {
            All(rules) => {
                core::All::new(rules.iter().map(|r| convert_serializable_rule(r, lang))).match_node_with_env(node, env)
            }
            Any(rules) => {
                core::Either::new(rules.into_iter().map(|r| convert_serializable_rule_to_positive(r, lang))).match_node_with_env(node, env)
            }
            Not(rule) => {
                core::Rule::not(convert_serializable_rule(rule, lang)).match_node_with_env(node, env)
            }
            Inside(rule) => {
                core::rule::Inside::new(convert_serializable_rule(rule, lang)).match_node_with_env(node, env)
            }
            Has(rule) => {
                core::rule::Inside::new(convert_serializable_rule(rule, lang)).match_node_with_env(node, env)
            }
            Pattern(pattern) => core::Pattern::new(&pattern, lang).match_node_with_env(node, env),
            Kind(kind_name) => core::KindMatcher::new(&kind_name, lang).match_node_with_env(node, env),
        }
    }
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


type Parsed = Rule<SupportLang, SerializableRule>;
pub fn from_yaml_string(yaml: &str) -> Result<AstGrepRuleConfig, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

enum SerializeError {
    YamlError(serde_yaml::Error),
    MissPositiveMatcher,
}

fn convert_serializable_rule_to_positive(rule: &SerializableRule, lang: SupportLang) -> Box<dyn PositiveMatcher<SupportLang>> {
    use SerializableRule::*;
    match rule {
        All(rules) => {
            Box::new(core::All::new(rules.into_iter().map(|r| convert_serializable_rule(r, lang))))
        }
        Any(rules) => {
            Box::new(core::Either::new(rules.into_iter().map(|r| convert_serializable_rule_to_positive(r, lang))))
        }
        Pattern(s) => Box::new(core::Pattern::new(&s, lang)),
        Kind(kind_name) => Box::new(core::KindMatcher::new(&kind_name, lang)),
        _ => panic!("impossible!"),
    }
}

fn convert_serializable_rule(rule: &SerializableRule, lang: SupportLang) -> Box<dyn Matcher<SupportLang>> {
    use SerializableRule::*;
    match rule {
        All(rules) => {
            Box::new(core::All::new(rules.into_iter().map(|r| convert_serializable_rule(r, lang))))
        }
        Any(rules) => {
            Box::new(core::Either::new(rules.into_iter().map(|r| convert_serializable_rule_to_positive(r, lang))))
        }
        Not(rule) => {
            Box::new(core::Rule::not(convert_serializable_rule(rule, lang)))
        }
        Inside(rule) => {
            Box::new(core::rule::Inside::new(convert_serializable_rule(rule, lang)))
        }
        Has(rule) => {
            Box::new(core::rule::Inside::new(convert_serializable_rule(rule, lang)))
        }
        Pattern(pattern) => Box::new(core::Pattern::new(&pattern, lang)),
        Kind(kind_name) => Box::new(core::KindMatcher::new(&kind_name, lang)),
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use ast_grep_core::language::Language;

    fn test_rule_match(yaml: &str, source: &str) {
        let config = from_yaml_string(yaml).expect("rule should parse");
        let grep = config.language.new(source);
        assert!(grep.root().find(config).is_some());
    }

    fn test_rule_unmatch(yaml: &str, source: &str) {
        let config = from_yaml_string(yaml).expect("rule should parse");
        let grep = config.language.new(source);
        assert!(grep.root().find(config).is_none());
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
