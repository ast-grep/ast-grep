use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_core::replacer::Fixer;
use ast_grep_core::traversal::Visitor;
use ast_grep_core::{Matcher, Pattern as SgPattern, StrDoc};
use ast_grep_language::Language;
use clap::Parser;
use ignore::WalkParallel;

use crate::config::{register_custom_language, IgnoreFile, NoIgnore};
use crate::error::ErrorContext as EC;
use crate::lang::SgLang;
use crate::print::{
  ColorArg, ColoredPrinter, Diff, Heading, InteractivePrinter, JSONPrinter, Printer,
};
use crate::utils::{filter_file_pattern, is_from_stdin, MatchUnit};
use crate::utils::{run_std_in, StdInWorker};
use crate::utils::{run_worker, Items, Worker};

type Pattern<L> = SgPattern<StrDoc<L>>;

// NOTE: have to register custom lang before clap read arg
// RunArg has a field of SgLang
pub fn register_custom_language_if_is_run(args: &[String]) {
  if !args.is_empty() || args[1].starts_with('-') || args[1] == "run" {
    register_custom_language(None);
  }
}

fn lang_help() -> String {
  format!(
    "The language of the pattern. Supported languages are:\n{:?}",
    SgLang::all_langs()
  )
}

const LANG_HELP_LONG: &str = "The language of the pattern. For full language list, visit https://ast-grep.github.io/reference/languages.html";

#[derive(Parser)]
pub struct RunArg {
  // search pattern related options
  /// AST pattern to match.
  #[clap(short, long)]
  pattern: String,

  /// String to replace the matched AST node.
  #[clap(short, long)]
  rewrite: Option<String>,

  /// Print query pattern's tree-sitter AST. Requires lang be set explicitly.
  #[clap(long, requires = "lang")]
  debug_query: bool,

  /// The language of the pattern query.
  #[clap(short, long, help(lang_help()), long_help=LANG_HELP_LONG)]
  lang: Option<SgLang>,

  // output related options
  /// Start interactive edit session. Code rewrite only happens inside a session.
  #[clap(short, long)]
  interactive: bool,

  /// Output matches in structured JSON text useful for tools like jq.
  /// Conflicts with interactive.
  #[clap(long, conflicts_with = "interactive")]
  json: bool,

  /// Print the file name as heading before all matches of that file.
  /// File path will be printed before each match as prefix if heading is disabled.
  /// This is the default mode when printing to a terminal.
  #[clap(long, default_value = "auto")]
  heading: Heading,

  /// Controls output color.
  #[clap(long, default_value = "auto")]
  color: ColorArg,

  // input related options
  /// The paths to search. You can provide multiple paths separated by spaces.
  #[clap(value_parser, default_value = ".")]
  paths: Vec<PathBuf>,

  /// Apply all rewrite without confirmation if true.
  #[clap(short = 'U', long, requires = "rewrite")]
  update_all: bool,

  /// Do not respect hidden file system or ignore files (.gitignore, .ignore, etc.).
  /// You can suppress multiple ignore files by passing `no-ignore` multiple times.
  #[clap(long, action = clap::ArgAction::Append)]
  no_ignore: Vec<IgnoreFile>,

  /// Enable search code from StdIn.
  ///
  /// Use this if you need to take code stream from standard input.
  /// If the environment variable `AST_GREP_NO_STDIN` exist, ast-grep will disable StdIn mode.
  #[clap(long)]
  stdin: bool,

  // context related options
  /// Show NUM lines after each match.
  #[clap(
    short = 'A',
    long,
    default_value = "0",
    conflicts_with = "context",
    value_name = "NUM"
  )]
  after: u16,
  /// Show NUM lines before each match.
  #[clap(
    short = 'B',
    long,
    default_value = "0",
    conflicts_with = "context",
    value_name = "NUM"
  )]
  before: u16,
  /// Show NUM lines around each match.
  #[clap(short = 'C', long, default_value = "0", value_name = "NUM")]
  context: u16,
}

// Every run will include Search or Replace
// Search or Replace by arguments `pattern` and `rewrite` passed from CLI
pub fn run_with_pattern(arg: RunArg) -> Result<()> {
  if arg.json {
    return run_pattern_with_printer(arg, JSONPrinter::stdout());
  }
  let context = if arg.context != 0 {
    (arg.context, arg.context)
  } else {
    (arg.before, arg.after)
  };
  let printer = ColoredPrinter::stdout(arg.color)
    .heading(arg.heading)
    .context(context);
  let interactive = arg.interactive || arg.update_all;
  if interactive {
    let from_stdin = arg.stdin && is_from_stdin();
    let printer = InteractivePrinter::new(printer, arg.update_all, from_stdin)?;
    run_pattern_with_printer(arg, printer)
  } else {
    run_pattern_with_printer(arg, printer)
  }
}

fn run_pattern_with_printer(arg: RunArg, printer: impl Printer + Sync) -> Result<()> {
  if arg.stdin && is_from_stdin() {
    run_std_in(RunWithSpecificLang::new(arg, printer)?)
  } else if arg.lang.is_some() {
    run_worker(RunWithSpecificLang::new(arg, printer)?)
  } else {
    run_worker(RunWithInferredLang { arg, printer })
  }
}

struct RunWithInferredLang<Printer> {
  arg: RunArg,
  printer: Printer,
}

impl<P: Printer + Sync> Worker for RunWithInferredLang<P> {
  type Item = (MatchUnit<Pattern<SgLang>>, SgLang);
  fn build_walk(&self) -> WalkParallel {
    let arg = &self.arg;
    let threads = num_cpus::get().min(12);
    NoIgnore::disregard(&arg.no_ignore)
      .walk(&arg.paths)
      .threads(threads)
      .build_parallel()
  }

  fn produce_item(&self, path: &Path) -> Option<Self::Item> {
    let lang = SgLang::from_path(path)?;
    let matcher = Pattern::try_new(&self.arg.pattern, lang).ok()?;
    let match_unit = filter_file_pattern(path, lang, matcher)?;
    Some((match_unit, lang))
  }

  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    let rewrite = &self.arg.rewrite;
    let printer = &self.printer;
    printer.before_print()?;
    for (match_unit, lang) in items {
      let rewrite = rewrite
        .as_ref()
        .map(|s| Fixer::try_new(s, &lang))
        .transpose();
      match rewrite {
        Ok(r) => match_one_file(printer, &match_unit, &r)?,
        Err(e) => {
          match_one_file(printer, &match_unit, &None)?;
          eprintln!("⚠️  Rewriting was skipped because pattern fails to parse. Error detail:");
          eprintln!("╰▻ {e}");
        }
      }
    }
    printer.after_print()?;
    Ok(())
  }
}

struct RunWithSpecificLang<Printer> {
  arg: RunArg,
  printer: Printer,
  pattern: Pattern<SgLang>,
}

impl<Printer> RunWithSpecificLang<Printer> {
  fn new(arg: RunArg, printer: Printer) -> Result<Self> {
    let pattern = &arg.pattern;
    let lang = arg.lang.ok_or(anyhow::anyhow!(EC::LanguageNotSpecified))?;
    let pattern = Pattern::try_new(pattern, lang).context(EC::ParsePattern)?;
    Ok(Self {
      arg,
      printer,
      pattern,
    })
  }
}

impl<P: Printer + Sync> Worker for RunWithSpecificLang<P> {
  type Item = MatchUnit<Pattern<SgLang>>;
  fn build_walk(&self) -> WalkParallel {
    let arg = &self.arg;
    let threads = num_cpus::get().min(12);
    let lang = arg.lang.expect("must present");
    NoIgnore::disregard(&arg.no_ignore)
      .walk(&arg.paths)
      .threads(threads)
      .types(lang.file_types())
      .build_parallel()
  }
  fn produce_item(&self, path: &Path) -> Option<Self::Item> {
    let arg = &self.arg;
    let pattern = self.pattern.clone();
    let lang = arg.lang.expect("must present");
    filter_file_pattern(path, lang, pattern)
  }
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    let printer = &self.printer;
    printer.before_print()?;
    let arg = &self.arg;
    let lang = arg.lang.expect("must present");
    if arg.debug_query {
      println!("Pattern TreeSitter {:?}", self.pattern);
    }
    let rewrite = if let Some(s) = &arg.rewrite {
      Some(Fixer::try_new(s, &lang).context(EC::ParsePattern)?)
    } else {
      None
    };
    for match_unit in items {
      match_one_file(printer, &match_unit, &rewrite)?;
    }
    printer.after_print()?;
    Ok(())
  }
}

impl<P: Printer + Sync> StdInWorker for RunWithSpecificLang<P> {
  fn parse_stdin(&self, src: String) -> Option<Self::Item> {
    let lang = self.arg.lang.expect("must present");
    let grep = lang.ast_grep(src);
    let has_match = grep.root().find(&self.pattern).is_some();
    has_match.then(|| MatchUnit {
      path: PathBuf::from("STDIN"),
      matcher: self.pattern.clone(),
      grep,
    })
  }
}

fn match_one_file(
  printer: &impl Printer,
  match_unit: &MatchUnit<impl Matcher<SgLang>>,
  rewrite: &Option<Fixer<String>>,
) -> Result<()> {
  let MatchUnit {
    path,
    grep,
    matcher,
  } = match_unit;

  let matches = Visitor::new(matcher).reentrant(false).visit(grep.root());
  if let Some(rewrite) = rewrite {
    let diffs = matches.map(|m| Diff::generate(m, matcher, rewrite));
    printer.print_diffs(diffs, path)
  } else {
    printer.print_matches(matches, path)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_language::SupportLang;

  fn default_run_arg() -> RunArg {
    RunArg {
      pattern: String::new(),
      rewrite: None,
      color: ColorArg::Never,
      no_ignore: vec![],
      stdin: false,
      interactive: false,
      lang: None,
      json: false,
      heading: Heading::Never,
      debug_query: false,
      update_all: false,
      paths: vec![PathBuf::from(".")],
      before: 0,
      after: 0,
      context: 0,
    }
  }

  #[test]
  fn test_run_with_pattern() {
    let arg = RunArg {
      pattern: "console.log".to_string(),
      ..default_run_arg()
    };
    assert!(run_with_pattern(arg).is_ok())
  }

  #[test]
  fn test_run_with_specific_lang() {
    let arg = RunArg {
      pattern: "Some(result)".to_string(),
      lang: Some(SupportLang::Rust.into()),
      stdin: true,
      ..default_run_arg()
    };
    assert!(run_with_pattern(arg).is_ok())
  }
}
