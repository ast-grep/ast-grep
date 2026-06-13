// Extraction and rendering will consume this model in later slices.
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod rule;

use std::process::ExitCode;

use clap::{Args, ValueEnum};

use crate::lang::SgLang;
use crate::print::{ColorArg, JsonStyle};
use crate::utils::InputArgs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutlineItems {
  /// Use `structure` for file or stdin input, `exports` when any directory is given.
  Auto,
  /// Top-level items defined locally in the file, excluding imports.
  Structure,
  /// Top-level items exported from the file or module.
  Exports,
  /// Top-level items imported from other files or modules.
  Imports,
  /// All top-level items, including imports and exports.
  All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutlineView {
  /// Use `digest` for file or stdin input, `names` when any directory is given.
  Auto,
  /// One grouped name line per symbol type for each file.
  Names,
  /// One source/signature line per top-level item.
  Signatures,
  /// Signatures plus compact member name digests.
  Digest,
  /// Signatures plus one source/signature line per direct member.
  Expanded,
}

#[derive(Args)]
pub struct OutlineArg {
  /// Specify the input language.
  ///
  /// For path input, ast-grep parses only files of this language. For stdin,
  /// this flag is required because there is no file path to infer the language from.
  #[clap(short, long, required_if_eq("stdin", "true"))]
  lang: Option<SgLang>,

  /// Output outline entries in structured JSON.
  ///
  /// If this flag is set, ast-grep will output outline entries in JSON format.
  /// You can pass optional value to this flag by using `--json=<STYLE>` syntax
  /// to further control how JSON object is formatted and printed. ast-grep will
  /// `pretty`-print JSON if no value is passed.
  /// Note, the json flag must use `=` to specify its value.
  #[clap(
      long,
      value_name="STYLE",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pretty"
  )]
  json: Option<JsonStyle>,

  /// Controls output color.
  ///
  /// This flag controls when to use colors. The default setting is 'auto', which
  /// means ast-grep will try to guess when to use colors. If ast-grep is
  /// printing to a terminal, then it will use colors, but if it is redirected to a
  /// file or a pipe, then it will suppress color output. ast-grep will also suppress
  /// color output in some other circumstances. For example, no color will be used
  /// if the TERM environment variable is not set or set to 'dumb'.
  #[clap(long, default_value = "auto", value_name = "WHEN")]
  color: ColorArg,

  /// Select which top-level items to include.
  ///
  /// This option controls top-level structure such as classes, structs, interfaces,
  /// functions, and modules. It does not filter members.
  /// By default, ast-grep picks the items automatically based on the input path.
  #[clap(long, default_value = "auto", value_name = "ITEMS")]
  items: OutlineItems,

  /// Keep only top-level items with these comma-separated symbol types.
  ///
  /// For example, `--type class,enum` keeps both classes and enums.
  #[clap(long = "type", value_name = "TYPE[,TYPE...]")]
  symbol_type: Option<String>,

  /// Keep only top-level items matching this regex.
  ///
  /// The regex is matched against item names, signatures, first source lines,
  /// and import/export item signatures. It never matches members.
  #[clap(long = "match", value_name = "REGEX")]
  match_item: Option<String>,

  /// Display only public members in member views.
  ///
  /// By default, member views display all extracted members; the digest view
  /// lists public members before non-public members.
  #[clap(long)]
  pub_members: bool,

  /// Select the text presentation.
  ///
  /// Views contain increasingly more information, from grouped names to expanded
  /// member signatures.
  /// By default, ast-grep picks the view automatically based on the input path.
  #[clap(long, default_value = "auto", value_name = "VIEW")]
  view: OutlineView,

  /// Input related options.
  #[clap(flatten)]
  input: InputArgs,
}

pub fn run_outline(arg: OutlineArg) -> anyhow::Result<ExitCode> {
  let _ = arg;
  println!("nothing found");
  Ok(ExitCode::SUCCESS)
}
