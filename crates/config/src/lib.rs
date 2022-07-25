pub mod support_language;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};
// use ast_grep_core::{Rule, Matcher};

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

// type Parsed = Rule<SupportLang, Box<dyn Matcher<SupportLang>>>;
// pub fn from_yaml_string(yaml: &str) -> Result<Parsed, serde_yaml::Error> {
//     let ast_grep_rule: AstGrepRuleConfig = serde_yaml::from_str(yaml);
//     todo!()
// }

#[cfg(test)]
mod test {
    #[test]
    fn test_deserialize_rule_config() {

    }
}
