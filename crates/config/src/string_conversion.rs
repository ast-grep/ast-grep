use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CaseConversion {
  LowerCase,
  UpperCase,
  Capitalize,
}

impl CaseConversion {
  pub fn apply(&self, string: String) -> String {
    match &self {
      CaseConversion::LowerCase => string.to_lowercase(),
      CaseConversion::UpperCase => string.to_uppercase(),
      CaseConversion::Capitalize => {
        let mut chars = string.chars();
        if let Some(c) = chars.next() {
          c.to_uppercase().chain(chars).collect()
        } else {
          string
        }
      }
    }
  }
}
