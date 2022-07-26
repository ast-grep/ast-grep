pub mod support_language;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use ast_grep_core::{Rule, Matcher, PositiveMatcher};
use ast_grep_core as core;

pub use support_language::SupportLang;


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct AstGrepRuleConfig {
    /// Unique, descriptive identifier, e.g., no-unused-variable
    id: String,
    /// Message highlighting why this rule fired and how to remediate the issue
    message: String,
    /// One of: Info, Warning, or Error
    severity: Severity,
    /// Specify the language to parse and the file extension to includ in matching.
    language: SupportLang,
    /// Pattern rules to find matching AST nodes
    rule: SerializableRule,
    /// Addtionaly meta variables pattern to filter matching
    #[serde(default)]
    meta_variables: HashMap<String, String>,
}

type Parsed = Rule<SupportLang, Box<dyn PositiveMatcher<SupportLang>>>;
pub fn from_yaml_string(yaml: &str) -> Result<Parsed, serde_yaml::Error> {
    let ast_grep_rule: AstGrepRuleConfig = serde_yaml::from_str(yaml)?;
    let matcher = convert_serializable_rule_to_positive(ast_grep_rule.rule, ast_grep_rule.language);
    Ok(Rule::new(matcher))
}

enum SerializeError {
    YamlError(serde_yaml::Error),
    MissPositiveMatcher,
}

fn convert_serializable_rule_to_positive(rule: SerializableRule, lang: SupportLang) -> Box<dyn PositiveMatcher<SupportLang>> {
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

fn convert_serializable_rule(rule: SerializableRule, lang: SupportLang) -> Box<dyn Matcher<SupportLang>> {
    use SerializableRule::*;
    match rule {
        All(rules) => {
            Box::new(core::All::new(rules.into_iter().map(|r| convert_serializable_rule(r, lang))))
        }
        Any(rules) => {
            Box::new(core::Either::new(rules.into_iter().map(|r| convert_serializable_rule_to_positive(r, lang))))
        }
        Not(rule) => {
            Box::new(core::Rule::not(convert_serializable_rule(*rule, lang)))
        }
        Inside(rule) => {
            Box::new(core::rule::Inside::new(convert_serializable_rule(*rule, lang)))
        }
        Has(rule) => {
            Box::new(core::rule::Inside::new(convert_serializable_rule(*rule, lang)))
        }
        Pattern(pattern) => Box::new(core::Pattern::new(&pattern, lang)),
        Kind(kind_name) => Box::new(core::KindMatcher::new(&kind_name, lang)),
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn test_rule_match(yaml: &str, source: &str) {
        let config = from_yaml_string(yaml).expect("rule should parse");
        let grep = core::AstGrep::new(source, SupportLang::TypeScript);
        assert!(grep.root().find(config).is_some());
    }

    fn test_rule_unmatch(yaml: &str, source: &str) {
        let config = from_yaml_string(yaml).expect("rule should parse");
        let grep = core::AstGrep::new(source, SupportLang::TypeScript);
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
