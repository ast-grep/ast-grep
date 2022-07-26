pub mod support_language;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use ast_grep_core::{Rule, Matcher, PositiveMatcher};
use ast_grep_core as core;

pub use support_language::SupportLang;


#[derive(Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize)]
pub enum SerializableRule {
    All(Vec<SerializableRule>),
    Any(Vec<SerializableRule>),
    Not(Box<SerializableRule>),
    Inside(Box<SerializableRule>),
    Has(Box<SerializableRule>),
    Pattern(String),
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
            Box::new(core::All::new(rules.into_iter().map(|r| convert_serializable_rule_to_positive(r, lang))))
        }
        Any(rules) => {
            Box::new(core::Either::new(rules.into_iter().map(|r| convert_serializable_rule_to_positive(r, lang))))
        }
        Pattern(s) => Box::new(core::Pattern::new(&s, lang)),
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
        Pattern(s) => Box::new(core::Pattern::new(&s, lang))
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_deserialize_rule_config() {

    }
}
