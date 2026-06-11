// Extraction and rendering will consume this model in later slices.
#[allow(dead_code)]
mod model;
// Builtin and custom extractors will consume this rule contract later.
#[allow(dead_code)]
mod rule;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, ValueEnum};

use crate::lang::SgLang;
use crate::print::{ColorArg, JsonStyle};
use crate::utils::InputArgs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutlineItems {
  Auto,
  Structure,
  Exports,
  Imports,
  All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutlineView {
  Auto,
  Names,
  Signatures,
  Digest,
  Expanded,
}

#[derive(Args)]
pub struct OutlineArg {
  /// Parse input as a specific language.
  ///
  /// This is required when reading from standard input. For path input,
  /// ast-grep infers the language from each file path unless this option is set.
  #[clap(short, long, required_if_eq("stdin", "true"))]
  lang: Option<SgLang>,

  /// Output outline entries as JSON.
  ///
  /// Use `--json` for pretty JSON, `--json=compact` for compact JSON, or
  /// `--json=stream` for newline-delimited entries. Text output remains the
  /// default for interactive use.
  #[clap(
      long,
      value_name="STYLE",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pretty"
  )]
  json: Option<JsonStyle>,

  /// Controls output color in text mode.
  #[clap(long, default_value = "auto", value_name = "WHEN")]
  color: ColorArg,

  /// Select which top-level items to include.
  #[clap(long, default_value = "auto", value_name = "ITEMS")]
  items: OutlineItems,

  /// Keep only top-level items with these comma-separated symbol types.
  ///
  /// Examples: `function`, `struct,enum`, `class,interface`.
  #[clap(long = "type", value_name = "TYPE[,TYPE...]")]
  symbol_type: Option<String>,

  /// Keep only top-level items whose useful fields match this regex.
  #[clap(long = "match", value_name = "REGEX")]
  match_item: Option<String>,

  /// Display only public members in member views.
  #[clap(long)]
  pub_members: bool,

  /// Select the text presentation.
  #[clap(long, default_value = "auto", value_name = "VIEW")]
  view: OutlineView,

  /// Load additional outline extractor definitions.
  #[clap(long, value_name = "FILE", action = clap::ArgAction::Append)]
  outline_rules: Vec<PathBuf>,

  /// Disable bundled outline extractor definitions.
  #[clap(long)]
  no_default_outline_rules: bool,

  /// Input related options.
  #[clap(flatten)]
  input: InputArgs,
}

pub fn run_outline(arg: OutlineArg) -> anyhow::Result<ExitCode> {
  let _ = arg;
  println!("nothing found");
  Ok(ExitCode::SUCCESS)
}
