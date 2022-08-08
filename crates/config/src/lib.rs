mod config_rule;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_yaml::Deserializer;

pub use config_rule::{try_from_serializable, Rule, SerializableRule};
use ast_grep_core::language::Language;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RuleConfig<L: Language> {
    /// Unique, descriptive identifier, e.g., no-unused-variable
    pub id: String,
    /// Main message highlighting why this rule fired. It should be single line and concise,
    /// but specific enough to be understood without additional context.
    pub message: String,
    /// Additional notes to elaborate the message and provide potential fix to the issue.
    pub note: Option<String>,
    /// One of: Info, Warning, or Error
    pub severity: Severity,
    /// Specify the language to parse and the file extension to includ in matching.
    pub language: L,
    /// Pattern rules to find matching AST nodes
    pub rule: SerializableRule,
    /// A pattern to auto fix the issue. It can reference metavariables appeared in rule.
    pub fix: Option<String>,
    /// Addtionaly meta variables pattern to filter matching
    #[serde(default)]
    pub meta_variables: HashMap<String, String>,
}

impl<L: Language> RuleConfig<L> {
    pub fn get_matcher(&self) -> Rule<L> {
        try_from_serializable(self.rule.clone(), self.language.clone()).unwrap()
    }
}

pub fn from_yaml_string<'a, L: Language + Deserialize<'a>>(yamls: &'a str) -> Result<Vec<RuleConfig<L>>, serde_yaml::Error> {
    let mut ret = vec![];
    for yaml in Deserializer::from_str(yamls) {
        let config = RuleConfig::deserialize(yaml)?;
        ret.push(config);
    }
    Ok(ret)
}

pub struct Configs<L: Language> {
    pub configs: Vec<RuleConfig<L>>,
}
impl<L: Language> Configs<L> {
    pub fn new(configs: Vec<RuleConfig<L>>) -> Self {
        Self { configs }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use ast_grep_core::language::TSLanguage;
    #[derive(Clone, Deserialize)]
    pub enum TypeScript {
        Tsx,
    }
    impl Language for TypeScript {
        fn get_ts_language(&self) -> TSLanguage {
            tree_sitter_typescript::language_tsx().into()
        }
    }

    fn test_rule_match(yaml: &str, source: &str) {
        let config = &from_yaml_string::<TypeScript>(yaml).expect("rule should parse")[0];
        let grep = config.language.new(source);
        assert!(grep.root().find(config.get_matcher()).is_some());
    }

    fn test_rule_unmatch(yaml: &str, source: &str) {
        let config = &from_yaml_string::<TypeScript>(yaml).expect("rule should parse")[0];
        let grep = config.language.new(source);
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
