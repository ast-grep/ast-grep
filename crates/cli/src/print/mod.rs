mod cloud_print;
mod colored_print;
mod interactive_print;
mod json_print;

use crate::lang::SgLang;
use ast_grep_config::RuleConfig;
use ast_grep_core::replacer::Fixer;
use ast_grep_core::{Matcher, NodeMatch as SgNodeMatch, StrDoc};

use anyhow::Result;
use clap::ValueEnum;

use std::borrow::Cow;
use std::path::Path;

pub use cloud_print::{CloudPrinter, Platform};
pub use codespan_reporting::files::SimpleFile;
pub use codespan_reporting::term::termcolor::ColorChoice;
pub use colored_print::{print_diff, ColoredPrinter, Heading, PrintStyles, ReportStyle};
pub use interactive_print::InteractivePrinter;
pub use json_print::JSONPrinter;

type NodeMatch<'a, L> = SgNodeMatch<'a, StrDoc<L>>;

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SgLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

pub trait Printer {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()>;
  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()>;
  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()>;
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()>;
  /// Run before all printing. One CLI will run this exactly once.
  #[inline]
  fn before_print(&self) -> Result<()> {
    Ok(())
  }
  /// Run after all printing. One CLI will run this exactly once.
  #[inline]
  fn after_print(&self) -> Result<()> {
    Ok(())
  }
}

#[derive(Clone)]
pub struct Diff<'n> {
  /// the matched node
  pub node_match: NodeMatch<'n, SgLang>,
  /// string content for the replacement
  pub replacement: Cow<'n, str>,
}

impl<'n> Diff<'n> {
  pub fn generate(
    node_match: NodeMatch<'n, SgLang>,
    matcher: &impl Matcher<SgLang>,
    rewrite: &Fixer<String>,
  ) -> Self {
    let edit = node_match.make_edit(matcher, rewrite);
    let replacement = String::from_utf8(edit.inserted_text).unwrap();
    let replacement = Cow::Owned(replacement);
    Self {
      node_match,
      replacement,
    }
  }
}

#[derive(ValueEnum, Clone, Copy)]
pub enum ColorArg {
  /// Try to use colors, but don't force the issue. If the output is piped to another program,
  /// or the console isn't available on Windows, or if TERM=dumb, or if `NO_COLOR` is defined,
  /// for example, then don't use colors.
  Auto,
  /// Try very hard to emit colors. This includes emitting ANSI colors
  /// on Windows if the console API is unavailable (not implemented yet).
  Always,
  /// Ansi is like Always, except it never tries to use anything other
  /// than emitting ANSI color codes.
  Ansi,
  /// Never emit colors.
  Never,
}

impl From<ColorArg> for ColorChoice {
  fn from(arg: ColorArg) -> ColorChoice {
    use ColorArg::*;
    match arg {
      Auto => {
        if atty::is(atty::Stream::Stdout) {
          ColorChoice::Auto
        } else {
          ColorChoice::Never
        }
      }
      Always => ColorChoice::Always,
      Ansi => ColorChoice::AlwaysAnsi,
      Never => ColorChoice::Never,
    }
  }
}
