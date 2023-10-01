use serde::{Deserialize, Serialize};

fn capitalize(string: &str) -> String {
  let mut chars = string.chars();
  if let Some(c) = chars.next() {
    c.to_uppercase().chain(chars).collect()
  } else {
    string.to_string()
  }
}

enum Separator {}

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

use StringFormat::*;

impl StringFormat {
  pub fn apply(&self, string: &str) -> String {
    match &self {
      LowerCase => string.to_lowercase(),
      UpperCase => string.to_uppercase(),
      Capitalize => capitalize(string),
      _ => todo!()
      // CamelCase => camelize(string),
      // SnakeCase => snake_case(string),
      // KebabCase => kebab_case(string),
      // PascalCase => pascalize(string),
    }
  }
}

/**
  Split string by
  * CaseChange
  * Dash
  * Dot
  * Slash
  * Space
  * Underscore
*/
fn split(s: &str) -> impl Iterator<Item = &str> {
  let delimiters = ['-', '.', '/', ' ', '_'];
  let mut chars = s.chars();
  let mut is_lower = true;
  let mut left = 0;
  let mut right = 0;
  std::iter::from_fn(move || {
    for c in chars.by_ref() {
      // normal delimiter
      if delimiters.contains(&c) {
        let range = left..right;
        left = right + 1;
        right = left;
        is_lower = false;
        return Some(&s[range]);
      }
      // case delimiter
      if is_lower && c.is_uppercase() {
        let range = left..right;
        left = right;
        right = left + c.len_utf8();
        is_lower = c.is_lowercase();
        return Some(&s[range]);
      }
      is_lower = c.is_lowercase();
      right += c.len_utf8();
    }
    if left < right && right <= s.len() {
      let range = left..right;
      left = right;
      Some(&s[range])
    } else {
      None
    }
  })
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
  const CAMEL: &str = "camelsLiveInTheDesert";
  const SNAKE: &str = "snakes_live_in_forests";
  const KEBAB: &str = "kebab-is-a-delicious-food";
  const PASCAl: &str = "PascalIsACoolGuy";
  const PATH: &str = "path/is/a/slashed/string";
  const DOT: &str = "www.dot.com";
  const URL: &str = "x.com/hd_nvim";

  fn assert_split(s: &str, v: &[&str]) {
    let actual: Vec<_> = split(s).collect();
    assert_eq!(v, actual)
  }

  #[test]
  fn test_split() {
    assert_split(CAMEL, &["camels", "Live", "In", "The", "Desert"]);
    assert_split(SNAKE, &["snakes", "live", "in", "forests"]);
    assert_split(KEBAB, &["kebab", "is", "a", "delicious", "food"]);
    assert_split(PASCAl, &["", "Pascal", "Is", "ACool", "Guy"]);
    assert_split(PATH, &["path", "is", "a", "slashed", "string"]);
    assert_split(DOT, &["www", "dot", "com"]);
    assert_split(URL, &["x", "com", "hd", "nvim"]);
  }
}
