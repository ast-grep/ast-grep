use serde::{Deserialize, Serialize};
use std::ops::Range;

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

const DELIMITER: &[char] = &['-', '.', '/', ' ', '_'];
#[derive(Default, PartialEq, Eq)]
enum DelimitState {
  #[default]
  Lower,
  OneUpper,
  MultiUpper(char),
}

#[derive(Default)]
struct Delimiter {
  left: usize,
  right: usize,
  state: DelimitState,
}
impl Delimiter {
  fn delimit(&mut self, c: char) -> Option<Range<usize>> {
    let Self { left, right, state } = self;
    use DelimitState::*;
    // normal delimiter
    if DELIMITER.contains(&c) {
      let range = *left..*right;
      *left = *right + 1;
      *right = *left;
      self.state = Lower;
      return Some(range);
    }
    // case delimiter, from lowercase to uppercase
    if *state == Lower && c.is_uppercase() {
      let range = *left..*right;
      *left = *right;
      *right = *left + c.len_utf8();
      self.state = OneUpper;
      return Some(range);
    }
    // case 2, consecutive UpperCases followed by lowercase
    // e.g. XMLHttp -> XML Http
    if let MultiUpper(last_char) = state {
      if c.is_lowercase() {
        let new_left = *right - last_char.len_utf8();
        let range = *left..new_left;
        *left = new_left;
        *right += c.len_utf8();
        self.state = Lower;
        return Some(range);
      }
    }
    if c.is_lowercase() {
      self.state = Lower;
    } else if *state == Lower {
      self.state = OneUpper;
    } else {
      self.state = MultiUpper(c);
    }
    *right += c.len_utf8();
    None
  }
  fn conclude(&mut self, len: usize) -> Option<Range<usize>> {
    let Self { left, right, .. } = self;
    if left < right && *right <= len {
      let range = *left..*right;
      *left = *right;
      Some(range)
    } else {
      None
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
  let mut chars = s.chars();
  let mut delimiter = Delimiter::default();
  std::iter::from_fn(move || {
    for c in chars.by_ref() {
      if let Some(range) = delimiter.delimit(c) {
        if range.start != range.end {
          return Some(&s[range]);
        }
      }
    }
    let range = delimiter.conclude(s.len())?;
    if range.start != range.end {
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
    assert_split(PASCAL, &["Pascal", "Is", "A", "Cool", "Guy"]);
    assert_split(PATH, &["path", "is", "a", "slashed", "string"]);
    assert_split(DOT, &["www", "dot", "com"]);
    assert_split(URL, &["x", "com", "hd", "nvim"]);
    assert_split("XMLHttpRequest", &["XML", "Http", "Request"]);
    assert_split("whatHTML", &["what", "HTML"]);
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
