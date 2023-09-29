use serde::{Deserialize, Serialize};

fn capitalize(string: &str) -> String {
  let mut chars = string.chars();
  if let Some(c) = chars.next() {
    c.to_uppercase().chain(chars).collect()
  } else {
    string.to_string()
  }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum StringFormat {
  LowerCase,
  UpperCase,
  Capitalize,
  CamelCase,
  SnakeCase,
  KebabCase,
  PascalCase,
}

impl StringFormat {
  pub fn apply(&self, string: &str) -> String {
    match &self {
      StringFormat::LowerCase => string.to_lowercase(),
      StringFormat::UpperCase => string.to_uppercase(),
      StringFormat::Capitalize => capitalize(string),
      _ => todo!(),
    }
  }
  // pub fn split(&self, string: &str) -> Vec<String> {
  //   match &self {
  //     StringFormat::CamelCase => split_by_capital_letters(string),
  //     StringFormat::SnakeCase => split_snake_case(string),
  //     StringFormat::KebabCase => split_kebab_case(string),
  //     StringFormat::PascalCase => split_by_capital_letters(string),
  //     _ => todo!(),
  //   }
  // }
  // pub fn join(&self, words: &[String]) -> String {
  //   match &self {
  //     StringFormat::CamelCase => join_camel_case(words),
  //     StringFormat::SnakeCase => words.join("_"),
  //     StringFormat::KebabCase => words.join("-"),
  //     StringFormat::PascalCase => words
  //       .iter()
  //       .map(|s| capitalize(s))
  //       .collect::<Vec<_>>()
  //       .join(""),
  //     _ => todo!(),
  //   }
  // }
}

// fn split_words(s: &str) -> Vec<String> {
//   s.split_whitespace().map(|s| s.to_string()).collect()
// }

// fn join_camel_case(words: &[String]) -> String {
//   let mut result = String::new();
//   for (i, word) in words.iter().enumerate() {
//     if i == 0 {
//       result.push_str(&word.to_lowercase());
//     } else {
//       result.push_str(&capitalize(word));
//     }
//   }
//   result
// }

// fn split_by_capital_letters(camel_string: &str) -> Vec<String> {
//   let mut words = vec![];
//   let mut word = String::new();
//   for c in camel_string.chars() {
//     if c.is_uppercase() {
//       if !word.is_empty() {
//         words.push(word);
//       }
//       word = String::new();
//     }
//     word.push(c.to_ascii_lowercase());
//   }
//   words.push(word);
//   words
// }

// fn split_snake_case(snake_string: &str) -> Vec<String> {
//   snake_string.split('_').map(|s| s.to_string()).collect()
// }

// fn split_kebab_case(kebab_string: &str) -> Vec<String> {
//   kebab_string.split('-').map(|s| s.to_string()).collect()
// }

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_case_conversions() {
    assert_eq!(StringFormat::LowerCase.apply("aBc"), "abc");
    assert_eq!(StringFormat::UpperCase.apply("aBc"), "ABC");
    assert_eq!(StringFormat::Capitalize.apply("aBc"), "ABc");
  }

  fn icc(from: StringFormat, to: StringFormat, input: &str, expected: &str) {
    // let conversion = IdentifierConventionConversion {
    //   from,
    //   to,
    //   letter_case: None,
    // };
    // assert_eq!(conversion.apply(&input.to_string()), expected);
  }

  fn icc_case(
    from: StringFormat,
    to: StringFormat,
    case: StringFormat,
    input: &str,
    expected: &str,
  ) {
    // let conversion = IdentifierConventionConversion {
    //   from,
    //   to,
    //   letter_case: Some(case),
    // };
    // assert_eq!(conversion.apply(&input.to_string()), expected);
  }

  #[test]
  fn test_identifier_convention_conversions() {
    icc(
      StringFormat::CamelCase,
      StringFormat::SnakeCase,
      "camelsLiveInTheDesert",
      "camels_live_in_the_desert",
    );
    icc(
      StringFormat::SnakeCase,
      StringFormat::KebabCase,
      "snakes_live_in_forests",
      "snakes-live-in-forests",
    );
    icc(
      StringFormat::KebabCase,
      StringFormat::PascalCase,
      "kebab-is-a-delicious-food",
      "KebabIsADeliciousFood",
    );
    icc(
      StringFormat::PascalCase,
      StringFormat::CamelCase,
      "PascalIsACoolGuy",
      "pascalIsACoolGuy",
    );
    icc_case(
      StringFormat::CamelCase,
      StringFormat::SnakeCase,
      StringFormat::UpperCase,
      "birdsAreLoudAnimals",
      "BIRDS_ARE_LOUD_ANIMALS",
    );
    icc_case(
      StringFormat::SnakeCase,
      StringFormat::KebabCase,
      StringFormat::Capitalize,
      "hiss_snarl_roar",
      "Hiss-Snarl-Roar",
    );
  }
}
