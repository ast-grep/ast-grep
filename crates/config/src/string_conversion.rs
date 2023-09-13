use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum StringConversion {
  LowerCase,
  UpperCase,
  Capitalize,
}

pub fn apply_string_conversion(string: String, transform: &StringConversion) -> String {
  match transform {
    StringConversion::LowerCase => string.to_lowercase(),
    StringConversion::UpperCase => string.to_uppercase(),
    StringConversion::Capitalize => {
      let mut chars = string.chars();
      if let Some(c) = chars.next() {
        c.to_uppercase().chain(chars).collect()
      } else {
        string
      }
    }
  }
}
