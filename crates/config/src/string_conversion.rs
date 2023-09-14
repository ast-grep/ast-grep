use serde::{Deserialize, Serialize};

fn capitalize(string: &String) -> String {
  let mut chars = string.chars();
  if let Some(c) = chars.next() {
    c.to_uppercase().chain(chars).collect()
  } else {
    string.to_string()
  }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CaseConversion {
  LowerCase,
  UpperCase,
  Capitalize,
}

impl CaseConversion {
  pub fn apply(&self, string: &String) -> String {
    match &self {
      CaseConversion::LowerCase => string.to_lowercase(),
      CaseConversion::UpperCase => string.to_uppercase(),
      CaseConversion::Capitalize => capitalize(string),
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IdentifierConvention {
  CamelCase,
  SnakeCase,
  KebabCase,
  PascalCase,
}

impl IdentifierConvention {
  pub fn split(&self, string: &String) -> Vec<String> {
    match &self {
      IdentifierConvention::CamelCase => split_by_capital_letters(&string),
      IdentifierConvention::SnakeCase => split_snake_case(&string),
      IdentifierConvention::KebabCase => split_kebab_case(&string),
      IdentifierConvention::PascalCase => split_by_capital_letters(&string),
    }
  }
  pub fn join(&self, words: &Vec<String>) -> String {
    match &self {
      IdentifierConvention::CamelCase => join_camel_case(&words),
      IdentifierConvention::SnakeCase => words.join("_"),
      IdentifierConvention::KebabCase => words.join("-"),
      IdentifierConvention::PascalCase => words.iter().map(|s| capitalize(s)).collect::<Vec<_>>().join(""),
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct IdentifierConventionConversion {
  from: IdentifierConvention,
  letter_case: Option<CaseConversion>,
  to: IdentifierConvention,
}

impl IdentifierConventionConversion {
  pub fn apply(&self, string: &String) -> String {
    let words = self.from.split(string);
    // Apply case conversion to each word individually,
    // for more flexibility when using e.g. snake or kebab case
    // as the destination.
    let words = if let Some(c) = self.letter_case {
      words.iter().map(|w| c.apply(w)).collect()
    } else {
      words
    };
    self.to.join(&words)
  }
}


fn join_camel_case(words: &Vec<String>) -> String {
  let mut result = String::new();
  for (i, word) in words.iter().enumerate() {
    if i == 0 {
      result.push_str(&word.to_lowercase());
    } else {
      result.push_str(&capitalize(word));
    }
  }
  result
}

fn split_by_capital_letters(camel_string: &String) -> Vec<String> {
  let mut words = vec![];
  let mut word = String::new();
  for c in camel_string.chars() {
    if c.is_uppercase() {
      if !word.is_empty() {
        words.push(word);
      }
      word = String::new();
    }
    word.push(c.to_ascii_lowercase());
  }
  words.push(word);
  words
}

fn split_snake_case(snake_string: &String) -> Vec<String> {
  snake_string.split('_').map(|s| s.to_string()).collect()
}

fn split_kebab_case(kebab_string: &String) -> Vec<String> {
  kebab_string.split('-').map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_case_conversions() {
    assert_eq!(CaseConversion::LowerCase.apply(&"aBc".to_string()), "abc");
    assert_eq!(CaseConversion::UpperCase.apply(&"aBc".to_string()), "ABC");
    assert_eq!(CaseConversion::Capitalize.apply(&"aBc".to_string()), "ABc");
  }

  fn icc(from: IdentifierConvention, to: IdentifierConvention, input: &str, expected: &str) {
    let conversion = IdentifierConventionConversion { from, to, letter_case: None };
    assert_eq!(conversion.apply(&input.to_string()), expected);
  }

  fn icc_case(from: IdentifierConvention, to: IdentifierConvention, case: CaseConversion, input: &str, expected: &str) {
    let conversion = IdentifierConventionConversion { from, to, letter_case: Some(case) };
    assert_eq!(conversion.apply(&input.to_string()), expected);
  }


  #[test]
  fn test_identifier_convention_conversions() {
    icc(IdentifierConvention::CamelCase, IdentifierConvention::SnakeCase, "camelsLiveInTheDesert", "camels_live_in_the_desert");
    icc(IdentifierConvention::SnakeCase, IdentifierConvention::KebabCase, "snakes_live_in_forests", "snakes-live-in-forests");
    icc(IdentifierConvention::KebabCase, IdentifierConvention::PascalCase, "kebab-is-a-delicious-food", "KebabIsADeliciousFood");
    icc(IdentifierConvention::PascalCase, IdentifierConvention::CamelCase, "PascalIsACoolGuy", "pascalIsACoolGuy");
    icc_case(IdentifierConvention::CamelCase, IdentifierConvention::SnakeCase, CaseConversion::UpperCase, "birdsAreLoudAnimals", "BIRDS_ARE_LOUD_ANIMALS");
    icc_case(IdentifierConvention::SnakeCase, IdentifierConvention::KebabCase, CaseConversion::Capitalize, "hiss_snarl_roar", "Hiss-Snarl-Roar");
  }
}