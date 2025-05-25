use super::rewrite::Rewrite;
use super::trans::{Convert, Replace, Substring};
use super::Trans;
use ast_grep_core::meta_var::MetaVariable;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseTransError {
  #[error("`{0}` has syntax error.")]
  Syntax(String),
  #[error("`{0}` is not a valid transformation.")]
  InvalidTransform(String),
}

impl FromStr for Trans<MetaVariable> {
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

fn to_convert(decomposed: DecomposedTransString) -> Result<Convert<MetaVariable>, ParseTransError> {
  todo!("Implement Convert parsing from decomposed string")
}

fn to_replace(decomposed: DecomposedTransString) -> Result<Replace<MetaVariable>, ParseTransError> {
  todo!("Implement Replace parsing from decomposed string")
}
fn to_substring(
  decomposed: DecomposedTransString,
) -> Result<Substring<MetaVariable>, ParseTransError> {
  todo!("Implement Substring parsing from decomposed string")
}
fn to_rewrite(decomposed: DecomposedTransString) -> Result<Rewrite<MetaVariable>, ParseTransError> {
  todo!("Implement Rewrite parsing from decomposed string")
}
