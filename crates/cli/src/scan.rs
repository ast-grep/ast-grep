use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_config::{RuleCollection, RuleConfig, RuleWithConstraint};
use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Matcher};
use clap::Args;
use ignore::WalkParallel;

use crate::config::{find_config, read_rule_file, IgnoreFile, NoIgnore};
use crate::error::ErrorContext as EC;
use crate::interaction::{run_worker, Items, Worker};
use crate::print::{
  ColorArg, ColoredPrinter, Diff, InteractivePrinter, JSONPrinter, Printer, ReportStyle, SimpleFile,
};
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

/// A single atomic unit where matches happen.
/// It contains the file path, sg instance and matcher.
/// An analogy to compilation unit in C programming language.
struct MatchUnit<M: Matcher<SupportLang>> {
  path: PathBuf,
  grep: AstGrep<SupportLang>,
  matcher: M,
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

const MAX_FILE_SIZE: usize = 3_000_000;
const MAX_LINE_COUNT: usize = 200_000;

fn file_too_large(file_content: &String) -> bool {
  file_content.len() > MAX_FILE_SIZE && file_content.lines().count() > MAX_LINE_COUNT
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

fn filter_file_interactive<M: Matcher<SupportLang>>(
  path: &Path,
  lang: SupportLang,
  matcher: M,
) -> Option<MatchUnit<M>> {
  let file_content = read_to_string(path)
    .with_context(|| format!("Cannot read file {}", path.to_string_lossy()))
    .map_err(|err| eprintln!("{err}"))
    .ok()?;
  // skip large files
  if file_too_large(&file_content) {
    // TODO add output
    return None;
  }
  let grep = lang.ast_grep(file_content);
  let has_match = grep.root().find(&matcher).is_some();
  has_match.then(|| MatchUnit {
    grep,
    path: path.to_path_buf(),
    matcher,
  })
}
