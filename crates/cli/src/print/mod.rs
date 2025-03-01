mod cloud_print;
mod colored_print;
mod interactive_print;
mod json_print;

use crate::lang::SgLang;
use ast_grep_config::{Fixer, RuleConfig};
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
pub use json_print::{JSONPrinter, JsonStyle};

type NodeMatch<'a> = SgNodeMatch<'a, StrDoc<SgLang>>;

pub trait Printer {
  fn print_rule(
    &mut self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()>;
  fn print_matches(&mut self, matches: Vec<NodeMatch>, path: &Path) -> Result<()>;
  fn print_diffs(&mut self, diffs: Vec<Diff>, path: &Path) -> Result<()>;
  fn print_rule_diffs(
    &mut self,
    diffs: Vec<(Diff, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<()>;
  /// Run before all printing. One CLI will run this exactly once.
  #[inline]
  fn before_print(&mut self) -> Result<()> {
    Ok(())
  }
  /// Run after all printing. One CLI will run this exactly once.
  #[inline]
  fn after_print(&mut self) -> Result<()> {
    Ok(())
  }
}

#[derive(Clone)]
pub struct Diff<'n> {
  /// the matched node
  pub node_match: NodeMatch<'n>,
  /// string content for the replacement
  pub replacement: Cow<'n, str>,
  pub range: std::ops::Range<usize>,
}

impl<'n> Diff<'n> {
  pub fn generate(
    node_match: NodeMatch<'n>,
    matcher: &impl Matcher<SgLang>,
    rewrite: &Fixer<SgLang>,
  ) -> Self {
    let edit = node_match.make_edit(matcher, rewrite);
    let replacement = String::from_utf8(edit.inserted_text).unwrap();
    let replacement = Cow::Owned(replacement);
    Self {
      node_match,
      replacement,
      range: edit.position..edit.position + edit.deleted_length,
    }
  }

  /// Returns the root doc source code
  /// N.B. this can be different from node.text() because
  /// tree-sitter's root Node may not start at the begining
  pub fn get_root_text(&self) -> &'n str {
    self.node_match.root().get_text()
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

impl ColorArg {
  pub fn should_use_color(self) -> bool {
    use colored_print::should_use_color;
    should_use_color(&self.into())
  }
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
