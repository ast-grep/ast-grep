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
pub enum StringCase {
  LowerCase,
  UpperCase,
  Capitalize,
  CamelCase,
  SnakeCase,
  KebabCase,
  PascalCase,
}

use StringCase::*;

impl StringCase {
  pub fn apply(&self, s: &str) -> String {
    match &self {
      LowerCase => s.to_lowercase(),
      UpperCase => s.to_uppercase(),
      Capitalize => capitalize(s),
      CamelCase => join_camel_case(split(s)),
      SnakeCase => join(split(s), '_'),
      KebabCase => join(split(s), '-'),
      PascalCase => split(s).map(capitalize).collect(),
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

fn join<'a, I>(mut words: I, sep: char) -> String
where
  I: Iterator<Item = &'a str>,
{
  let mut result = String::new();
  if let Some(w) = words.next() {
    result.push_str(&w.to_lowercase());
  }
  for w in words {
    result.push(sep);
    result.push_str(&w.to_lowercase());
  }
  result
}

fn join_camel_case<'a, I>(words: I) -> String
where
  I: Iterator<Item = &'a str>,
{
  let mut result = String::new();
  for (i, word) in words.enumerate() {
    if i == 0 {
      result.push_str(&word.to_lowercase());
    } else {
      result.push_str(&capitalize(word));
    }
  }
  result
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_case_conversions() {
    assert_eq!(StringCase::LowerCase.apply("aBc"), "abc");
    assert_eq!(StringCase::UpperCase.apply("aBc"), "ABC");
    assert_eq!(StringCase::Capitalize.apply("aBc"), "ABc");
  }
  const CAMEL: &str = "camelsLiveInTheDesert";
  const SNAKE: &str = "snakes_live_in_forests";
  const KEBAB: &str = "kebab-is-a-delicious-food";
  const PASCAL: &str = "PascalIsACoolGuy";
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
    assert_split(PASCAL, &["", "Pascal", "Is", "ACool", "Guy"]);
    assert_split(PATH, &["path", "is", "a", "slashed", "string"]);
    assert_split(DOT, &["www", "dot", "com"]);
    assert_split(URL, &["x", "com", "hd", "nvim"]);
  }

  fn assert_format(fmt: StringCase, src: &str, expected: &str) {
    assert_eq!(fmt.apply(src), expected)
  }

  #[test]
  fn test_format() {
    assert_format(SnakeCase, CAMEL, "camels_live_in_the_desert");
    assert_format(KebabCase, CAMEL, "camels-live-in-the-desert");
    assert_format(PascalCase, KEBAB, "KebabIsADeliciousFood");
    assert_format(PascalCase, SNAKE, "SnakesLiveInForests");
  }
}
