use super::rewrite::Rewrite;
use super::trans::{Convert, Replace, Substring};
use super::Trans;
use serde_yaml::from_str as yaml_from_str;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseTransError {
  #[error("`{0}` has syntax error.")]
  Syntax(String),
  #[error("`{0}` is not a valid transformation.")]
  InvalidTransform(String),
  #[error("`{0}` is not a valid argument.")]
  InvalidArg(String),
  #[error("Argument `{0}` is required.")]
  RequiredArg(&'static str),
  #[error("Invalid argument value.")]
  ArgValue(#[from] serde_yaml::Error),
}

impl FromStr for Trans<String> {
  type Err = ParseTransError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let decomposed = decompose_str(s)?;
    let trans = match decomposed.func {
      "convert" => Trans::Convert(to_convert(decomposed)?),
      "replace" => Trans::Replace(to_replace(decomposed)?),
      "substring" => Trans::Substring(to_substring(decomposed)?),
      "rewrite" => Trans::Rewrite(to_rewrite(decomposed)?),
      invalid => return Err(ParseTransError::InvalidTransform(invalid.to_string())),
    };
    Ok(trans)
  }
}

struct DecomposedTransString<'a> {
  func: &'a str,
  source: &'a str,
  args: Vec<(&'a str, &'a str)>,
}

fn decompose_str(input: &str) -> Result<DecomposedTransString<'_>, ParseTransError> {
  let error = || ParseTransError::Syntax(input.to_string());
  let input = input.trim();
  let (func, rest) = input.split_once('(').ok_or_else(error)?;
  let func = func.trim();
  let rest = rest.trim_end_matches(')');
  let (source, rest) = rest.split_once(',').ok_or_else(error)?;
  let source = source.trim();
  let args = decompose_args(rest.trim()).ok_or_else(error)?;
  Ok(DecomposedTransString { func, source, args })
}

fn decompose_args(mut rest: &str) -> Option<Vec<(&str, &str)>> {
  let mut args = Vec::new();
  while !rest.is_empty() {
    let (key, next) = rest.split_once('=')?;
    let next = next.trim_start();
    let end_index = if next.starts_with(['\'', '"', '[']) {
      let end_char = match next.as_bytes()[0] {
        b'[' => ']',
        b => b as char,
      };
      next[1..].find(end_char)? + 1
    } else {
      next.find(',').unwrap_or(next.len()) - 1
    };
    let (val, next) = next.split_at(end_index + 1);
    // value should not be trimmed
    args.push((key.trim(), val));
    rest = next.trim_start().trim_start_matches(',').trim();
  }
  Some(args)
}

fn to_convert(decomposed: DecomposedTransString) -> Result<Convert<String>, ParseTransError> {
  debug_assert_eq!(decomposed.func, "convert");
  let mut to_case = None;
  let mut separated_by = None;
  for (key, value) in decomposed.args {
    match key {
      "toCase" => to_case = Some(value),
      "separatedBy" => separated_by = Some(value),
      _ => return Err(ParseTransError::InvalidArg(key.to_string())),
    }
  }
  let to_case = to_case.ok_or(ParseTransError::RequiredArg("to_case"))?;
  let to_case = yaml_from_str(to_case)?;
  let separated_by = separated_by.map(yaml_from_str).transpose()?;
  Ok(Convert {
    source: decomposed.source.to_string(),
    to_case,
    separated_by,
  })
}

fn to_replace(decomposed: DecomposedTransString) -> Result<Replace<String>, ParseTransError> {
  debug_assert_eq!(decomposed.func, "replace");
  let mut replace = None;
  let mut by = None;
  for (key, value) in decomposed.args {
    match key {
      "replace" => replace = Some(value),
      "by" => by = Some(value),
      _ => return Err(ParseTransError::InvalidArg(key.to_string())),
    }
  }
  let replace = replace.ok_or(ParseTransError::RequiredArg("replace"))?;
  let by = by.ok_or(ParseTransError::RequiredArg("by"))?;
  Ok(Replace {
    source: decomposed.source.to_string(),
    replace: serde_yaml::from_str(replace)?,
    by: serde_yaml::from_str(by)?,
  })
}
fn to_substring(decomposed: DecomposedTransString) -> Result<Substring<String>, ParseTransError> {
  debug_assert_eq!(decomposed.func, "substring");
  let mut start_char = None;
  let mut end_char = None;
  for (key, value) in decomposed.args {
    match key {
      "startChar" => start_char = Some(value),
      "endChar" => end_char = Some(value),
      _ => return Err(ParseTransError::InvalidArg(key.to_string())),
    }
  }
  let start_char = start_char.map(yaml_from_str).transpose()?;
  let end_char = end_char.map(yaml_from_str).transpose()?;
  Ok(Substring {
    source: decomposed.source.to_string(),
    start_char,
    end_char,
  })
}
fn to_rewrite(decomposed: DecomposedTransString) -> Result<Rewrite<String>, ParseTransError> {
  debug_assert_eq!(decomposed.func, "rewrite");
  let mut rewriters = None;
  let mut join_by = None;
  for (key, value) in decomposed.args {
    match key {
      "rewriters" => rewriters = Some(value),
      "joinBy" => join_by = Some(value),
      _ => return Err(ParseTransError::InvalidArg(key.to_string())),
    }
  }
  let rewriters = rewriters.ok_or(ParseTransError::RequiredArg("rewriters"))?;
  let rewriters = yaml_from_str(rewriters)?;
  Ok(Rewrite {
    source: decomposed.source.to_string(),
    rewriters,
    join_by: join_by.map(yaml_from_str).transpose()?,
  })
}

#[cfg(test)]
mod test {
  use crate::transform::string_case::StringCase;

  use super::*;

  #[test]
  fn test_decompose_str() {
    let input = "substring($A, startChar=1, endChar=2)";
    let decomposed = decompose_str(input).expect("should parse");
    assert_eq!(decomposed.func, "substring");
    assert_eq!(decomposed.source, "$A");
    assert_eq!(decomposed.args.len(), 2);
    assert_eq!(decomposed.args[0], ("startChar", "1"));
    assert_eq!(decomposed.args[1], ("endChar", "2"));
  }
  const SUBSTRING_CASE: &str = "substring($A, startChar=1, endChar=2)";
  const REPLACE_CASE: &str = "replace($A, replace= ^.+, by=', ')";
  const CONVERT_CASE: &str = "convert($A, toCase=camelCase, separatedBy=[underscore, dash])";
  const REWRITE_CASE: &str = "rewrite($A, rewriters=[rule1, rule2], joinBy = ',,,,')";

  #[test]
  fn test_decompose_cases() {
    let cases = [SUBSTRING_CASE, REPLACE_CASE, CONVERT_CASE, REWRITE_CASE];
    for case in cases {
      let decomposed = decompose_str(case).expect("should parse");
      match decomposed.func {
        "convert" => assert_eq!(decomposed.args.len(), 2),
        "replace" => assert_eq!(decomposed.args.len(), 2),
        "substring" => assert_eq!(decomposed.args.len(), 2),
        "rewrite" => assert_eq!(decomposed.args.len(), 2),
        _ => panic!("Unexpected function: {}", decomposed.func),
      }
    }
  }

  #[test]
  fn test_valid_transform() {
    let cases = [
      "convert($A, toCase=camelCase, separatedBy=[])",
      "replace($A, replace= ^.+, by =  '[')",
      "substring(   $A, startChar=1)",
      "substring(  $A,)",
      "rewrite($A, rewriters=[rule1, rule2])",
    ];
    for case in cases {
      Trans::from_str(case).expect("should parse convert");
    }
  }

  #[test]
  fn test_parse_convert() {
    let convert = Trans::from_str(CONVERT_CASE).expect("should parse convert");
    let Trans::Convert(convert) = convert else {
      panic!("Expected Convert transformation");
    };
    assert_eq!(convert.source, "$A");
    assert_eq!(convert.separated_by.map(|v| v.len()), Some(2));
    assert!(matches!(convert.to_case, StringCase::CamelCase));
  }

  #[test]
  fn test_parse_replace() {
    let replace = Trans::from_str(REPLACE_CASE).expect("should parse replace");
    let Trans::Replace(replace) = replace else {
      panic!("Expected Replace transformation");
    };
    assert_eq!(replace.source, "$A");
    assert_eq!(replace.replace, "^.+");
    assert_eq!(replace.by, ", ");
  }

  #[test]
  fn test_parse_substring() {
    let substring = Trans::from_str(SUBSTRING_CASE).expect("should parse substring");
    let Trans::Substring(substring) = substring else {
      panic!("Expected Substring transformation");
    };
    assert_eq!(substring.source, "$A");
    assert_eq!(substring.start_char, Some(1));
    assert_eq!(substring.end_char, Some(2));
  }

  #[test]
  fn test_parse_rewrite() {
    let rewrite = Trans::from_str(REWRITE_CASE).expect("should parse rewrite");
    let Trans::Rewrite(rewrite) = rewrite else {
      panic!("Expected Rewrite transformation");
    };
    assert_eq!(rewrite.source, "$A");
    assert_eq!(
      rewrite.rewriters,
      vec!["rule1".to_owned(), "rule2".to_owned()]
    );
    assert_eq!(rewrite.join_by, Some(",,,,".into()));
  }
}
