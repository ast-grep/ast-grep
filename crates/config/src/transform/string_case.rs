use schemars::JsonSchema;
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

/// An enumeration representing different cases for strings.
#[derive(Serialize, Deserialize, Clone, Copy, JsonSchema)]
#[serde(rename_all = "camelCase")]
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
  pub fn apply(&self, s: &str, seps: Option<&[Separator]>) -> String {
    match &self {
      LowerCase => s.to_lowercase(),
      UpperCase => s.to_uppercase(),
      Capitalize => capitalize(s),
      CamelCase => join_camel_case(split(s, seps)),
      SnakeCase => join(split(s, seps), '_'),
      KebabCase => join(split(s, seps), '-'),
      PascalCase => split(s, seps).map(capitalize).collect(),
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Copy, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Separator to split string. e.g. `user_accountName` -> `user`, `accountName`
/// It will be rejoin according to `StringCase`.
pub enum Separator {
  CaseChange,
  Dash,
  Dot,
  Slash,
  Space,
  Underscore,
}

impl From<&[Separator]> for Delimiter {
  fn from(value: &[Separator]) -> Self {
    use Separator::*;
    let mut delimiter = vec![];
    let mut state = CaseState::IgnoreCase;
    value.iter().for_each(|v| match v {
      CaseChange => state = CaseState::Lower,
      Dash => delimiter.push('-'),
      Dot => delimiter.push('.'),
      Slash => delimiter.push('/'),
      Space => delimiter.push(' '),
      Underscore => delimiter.push('_'),
    });
    Self {
      left: 0,
      right: 0,
      state,
      delimiter,
    }
  }
}

#[derive(PartialEq, Eq)]
/// CaseState is used to record the case change between two characters.
/// It will be used if separator is CaseChange.
enum CaseState {
  Lower,
  OneUpper,
  /// MultiUpper records consecutive uppercase characters.
  /// char is the last uppercase char, used to calculate the split range.
  MultiUpper(char),
  IgnoreCase,
}

struct Delimiter {
  left: usize,
  right: usize,
  state: CaseState,
  delimiter: Vec<char>,
}
impl Delimiter {
  fn all() -> Delimiter {
    Delimiter {
      left: 0,
      right: 0,
      state: CaseState::Lower,
      delimiter: vec!['-', '.', '/', ' ', '_'],
    }
  }
  fn delimit(&mut self, c: char) -> Option<Range<usize>> {
    let Self {
      left,
      right,
      state,
      delimiter,
    } = self;
    use CaseState::*;
    // normal delimiter
    if delimiter.contains(&c) {
      let range = *left..*right;
      *left = *right + 1;
      *right = *left;
      if *state != IgnoreCase {
        self.state = Lower;
      }
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
    *right += c.len_utf8();
    if *state == CaseState::IgnoreCase {
      return None;
    } else if c.is_lowercase() {
      self.state = Lower;
    } else if *state == Lower {
      self.state = OneUpper;
    } else {
      self.state = MultiUpper(c);
    }
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
  Split string by Separator
*/
fn split<'a>(s: &'a str, seps: Option<&[Separator]>) -> impl Iterator<Item = &'a str> {
  let mut chars = s.chars();
  let mut delimiter = if let Some(seps) = seps {
    Delimiter::from(seps)
  } else {
    Delimiter::all()
  };
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
    assert_eq!(StringCase::LowerCase.apply("aBc", None), "abc");
    assert_eq!(StringCase::UpperCase.apply("aBc", None), "ABC");
    assert_eq!(StringCase::Capitalize.apply("aBc", None), "ABc");
  }
  const CAMEL: &str = "camelsLiveInTheDesert";
  const SNAKE: &str = "snakes_live_in_forests";
  const KEBAB: &str = "kebab-is-a-delicious-food";
  const PASCAL: &str = "PascalIsACoolGuy";
  const PATH: &str = "path/is/a/slashed/string";
  const DOT: &str = "www.dot.com";
  const URL: &str = "x.com/hd_nvim";

  fn assert_split(s: &str, v: &[&str]) {
    let actual: Vec<_> = split(s, None).collect();
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

  fn assert_split_sep(s: &str, seps: &[Separator], v: &[&str]) {
    let actual: Vec<_> = split(s, Some(seps)).collect();
    assert_eq!(v, actual)
  }

  #[test]
  fn test_split_by_separator() {
    use Separator::*;
    assert_split_sep("user_accountName", &[Underscore], &["user", "accountName"]);
    assert_split_sep("user_accountName", &[Space], &["user_accountName"]);
    assert_split_sep("user_accountName", &[CaseChange], &["user_account", "Name"]);
  }

  fn assert_format(fmt: StringCase, src: &str, expected: &str) {
    assert_eq!(fmt.apply(src, None), expected)
  }

  #[test]
  fn test_format() {
    assert_format(SnakeCase, CAMEL, "camels_live_in_the_desert");
    assert_format(KebabCase, CAMEL, "camels-live-in-the-desert");
    assert_format(PascalCase, KEBAB, "KebabIsADeliciousFood");
    assert_format(PascalCase, SNAKE, "SnakesLiveInForests");
  }
}
