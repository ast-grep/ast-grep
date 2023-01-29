use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_config::{RuleCollection, RuleConfig, RuleWithConstraint};
use ast_grep_core::{AstGrep, Matcher};
use clap::Args;
use ignore::WalkParallel;

use crate::config::{find_config, read_rule_file, IgnoreFile, NoIgnore};
use crate::error::ErrorContext as EC;
use crate::print::{
  ColorArg, ColoredPrinter, Diff, InteractivePrinter, JSONPrinter, Printer, ReportStyle, SimpleFile,
};
use crate::utils::{filter_file_interactive, MatchUnit};
use crate::utils::{run_worker, Items, Worker};
use ast_grep_language::SupportLang;

#[derive(Args)]
pub struct ScanArg {
  /// Path to ast-grep root config, default is sgconfig.yml.
  #[clap(short, long)]
  config: Option<PathBuf>,

  /// Scan the codebase with one specified rule, without project config setup.
  #[clap(short, long, conflicts_with = "config")]
  rule: Option<PathBuf>,

  /// Start interactive edit session. Code rewrite only happens inside a session.
  #[clap(short, long, conflicts_with = "json")]
  interactive: bool,

  /// Controls output color.
  #[clap(long, default_value = "auto")]
  color: ColorArg,

  #[clap(long, default_value = "rich")]
  report_style: ReportStyle,

  /// Output matches in structured JSON text. This is useful for tools like jq.
  /// Conflicts with color and report-style.
  #[clap(long, conflicts_with = "color", conflicts_with = "report_style")]
  json: bool,

  /// Apply all rewrite without confirmation if true.
  #[clap(long)]
  accept_all: bool,

  /// The paths to search. You can provide multiple paths separated by spaces.
  #[clap(value_parser, default_value = ".")]
  paths: Vec<PathBuf>,

  /// Do not respect ignore files. You can suppress multiple ignore files by passing `no-ignore` multiple times.
  #[clap(long, action = clap::ArgAction::Append)]
  no_ignore: Vec<IgnoreFile>,
}

impl<'a> MatchUnit<&'a RuleWithConstraint<SupportLang>> {
  fn reuse_with_matcher(self, matcher: &'a RuleWithConstraint<SupportLang>) -> Self {
    Self { matcher, ..self }
  }
}

pub fn run_with_config(arg: ScanArg) -> Result<()> {
  if arg.json {
    let worker = ScanWithConfig::try_new(arg, JSONPrinter::stdout())?;
    return run_worker(worker);
  }
  let printer = ColoredPrinter::stdout(arg.color).style(arg.report_style);
  let interactive = arg.interactive || arg.accept_all;
  if interactive {
    let printer = InteractivePrinter::new(printer).accept_all(arg.accept_all);
    let worker = ScanWithConfig::try_new(arg, printer)?;
    run_worker(worker)
  } else {
    let worker = ScanWithConfig::try_new(arg, printer)?;
    run_worker(worker)
  }
}

struct ScanWithConfig<Printer> {
  arg: ScanArg,
  printer: Printer,
  configs: RuleCollection<SupportLang>,
}
impl<P: Printer> ScanWithConfig<P> {
  fn try_new(mut arg: ScanArg, printer: P) -> Result<Self> {
    let configs = if let Some(path) = &arg.rule {
      let rules = read_rule_file(path)?;
      RuleCollection::try_new(rules).context(EC::GlobPattern)?
    } else {
      find_config(arg.config.take())?
    };
    Ok(Self {
      arg,
      printer,
      configs,
    })
  }
}

impl<P: Printer + Sync> Worker for ScanWithConfig<P> {
  type Item = (PathBuf, AstGrep<SupportLang>);
  fn build_walk(&self) -> WalkParallel {
    let arg = &self.arg;
    let threads = num_cpus::get().min(12);
    NoIgnore::disregard(&arg.no_ignore)
      .walk(&arg.paths)
      .threads(threads)
      .build_parallel()
  }
  fn produce_item(&self, path: &Path) -> Option<Self::Item> {
    for config in &self.configs.for_path(path) {
      let lang = config.language;
      let matcher = &config.matcher;
      // TODO: we are filtering multiple times here, perf sucks :(
      let ret = filter_file_interactive(path, lang, matcher);
      if let Some(unit) = ret {
        return Some((unit.path, unit.grep));
      }
    }
    None
  }
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    for (path, grep) in items {
      let mut match_unit = MatchUnit {
        path,
        grep,
        matcher: &RuleWithConstraint::default(),
      };
      let path = &match_unit.path;
      let file_content = read_to_string(path)?;
      for config in self.configs.for_path(path) {
        let matcher = &config.matcher;
        // important reuse and mutation start!
        match_unit = match_unit.reuse_with_matcher(matcher);
        // important reuse and mutation end!
        match_rule_on_file(&match_unit, config, &file_content, &self.printer)?;
      }
    }
    self.printer.after_print()?;
    Ok(())
  }
}

fn match_rule_on_file(
  match_unit: &MatchUnit<impl Matcher<SupportLang>>,
  rule: &RuleConfig<SupportLang>,
  file_content: &String,
  reporter: &impl Printer,
) -> Result<()> {
  let MatchUnit {
    path,
    grep,
    matcher,
  } = match_unit;
  let mut matches = grep.root().find_all(matcher).peekable();
  if matches.peek().is_none() {
    return Ok(());
  }
  let file = SimpleFile::new(path.to_string_lossy(), file_content);
  if let Some(fixer) = &rule.fixer {
    let diffs = matches.map(|m| Diff::generate(m, matcher, fixer));
    reporter.print_rule_diffs(diffs, path, rule)?;
  } else {
    reporter.print_rule(matches, file, rule)?;
  }
  Ok(())
}
