pub mod support_language;

use serde::{Serialize, Deserialize};

pub use support_language::SupportLang;


#[derive(Serialize, Deserialize)]
enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize)]
struct AstGrepRuleConfig {
    /// Unique, descriptive identifier, e.g., no-unused-variable
    id: String,
    /// Message highlighting why this rule fired and how to remediate the issue
    message: String,
    /// One of: Info, Warning, or Error
    severity: Severity,
}

#[cfg(test)]
mod test {
    #[test]
    fn test_deserialize_rule_config() {

    }
}
