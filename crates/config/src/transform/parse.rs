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

fn decompose_str(input: &str) -> Result<DecomposedTransString, ParseTransError> {
  let error = || ParseTransError::Syntax(input.to_string());
  let input = input.trim();
  let (func, rest) = input.split_once('(').ok_or_else(error)?;
  let func = func.trim();
  let rest = rest.trim_end_matches(')');
  let mut rest = rest.split(',');
  let source = rest.next().ok_or_else(error)?.trim();
  let mut args = Vec::new();
  for arg_pair in rest {
    let (key, value) = arg_pair.split_once('=').ok_or_else(error)?;
    args.push((key.trim(), value.trim()));
  }
  Ok(DecomposedTransString { func, source, args })
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
    replace: replace.to_string(),
    by: by.to_string(),
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
    join_by: join_by.map(ToString::to_string),
  })
}
