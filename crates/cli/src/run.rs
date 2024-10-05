use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_config::Fixer;
use ast_grep_core::{MatchStrictness, Matcher, Pattern};
use ast_grep_language::Language;
use clap::{builder::PossibleValue, Parser, ValueEnum};
use ignore::WalkParallel;

use crate::config::register_custom_language;
use crate::lang::SgLang;
use crate::print::{ColoredPrinter, Diff, Heading, InteractivePrinter, JSONPrinter, Printer};
use crate::utils::ErrorContext as EC;
use crate::utils::{filter_file_pattern, InputArgs, MatchUnit, OutputArgs};
use crate::utils::{DebugFormat, FileStats, RunStats};
use crate::utils::{Items, PathWorker, StdInWorker, Worker};

// NOTE: have to register custom lang before clap read arg
// RunArg has a field of SgLang
pub fn register_custom_language_if_is_run(args: &[String]) -> Result<()> {
  let Some(arg) = args.get(1) else {
    return Ok(());
  };
  if arg.starts_with('-') || arg == "run" {
    register_custom_language(None)?;
  }
  Ok(())
}

fn lang_help() -> String {
  format!(
    "The language of the pattern. Supported languages are:\n{:?}",
    SgLang::all_langs()
  )
}

const LANG_HELP_LONG: &str = "The language of the pattern. For full language list, visit https://ast-grep.github.io/reference/languages.html";

#[derive(Clone)]
struct Strictness(MatchStrictness);
impl ValueEnum for Strictness {
  fn value_variants<'a>() -> &'a [Self] {
    use MatchStrictness as M;
    &[
      Strictness(M::Cst),
      Strictness(M::Smart),
      Strictness(M::Ast),
      Strictness(M::Relaxed),
      Strictness(M::Signature),
    ]
  }
  fn to_possible_value(&self) -> Option<PossibleValue> {
    use MatchStrictness as M;
    Some(match &self.0 {
      M::Cst => PossibleValue::new("cst").help("Match exact all node"),
      M::Smart => PossibleValue::new("smart").help("Match all node except source trivial nodes"),
      M::Ast => PossibleValue::new("ast").help("Match only ast nodes"),
      M::Relaxed => PossibleValue::new("relaxed").help("Match ast node except comments"),
      M::Signature => {
        PossibleValue::new("signature").help("Match ast node except comments, without text")
      }
    })
  }
}

#[derive(Parser)]
pub struct RunArg {
  // search pattern related options
  /// AST pattern to match.
  #[clap(short, long)]
  pattern: String,

  /// AST kind to extract sub-part of pattern to match.
  ///
  /// selector defines the sub-syntax node kind that is the actual matcher of the pattern.
  /// See https://ast-grep.github.io/guide/rule-config/atomic-rule.html#pattern-object.
  #[clap(long, value_name = "KIND")]
  selector: Option<String>,

  /// String to replace the matched AST node.
  #[clap(short, long, value_name = "FIX", required_if_eq("update_all", "true"))]
  rewrite: Option<String>,

  /// The language of the pattern query.
  #[clap(short, long, help(lang_help()), long_help=LANG_HELP_LONG)]
  lang: Option<SgLang>,

  /// Print query pattern's tree-sitter AST. Requires lang be set explicitly.
  #[clap(
      long,
      requires = "lang",
      value_name="format",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pattern"
  )]
  debug_query: Option<DebugFormat>,

  /// The strictness of the pattern.
  #[clap(long)]
  strictness: Option<Strictness>,

  /// input related options
  #[clap(flatten)]
  input: InputArgs,

  /// output related options
  #[clap(flatten)]
  output: OutputArgs,

  /// Controls whether to print the file name as heading.
  ///
  /// If heading is used, the file name will be printed as heading before all matches of that file.
  /// If heading is not used, ast-grep will print the file path before each match as prefix.
  /// The default value `auto` is to use heading when printing to a terminal
  /// and to disable heading when piping to another program or redirected to files.
  #[clap(long, default_value = "auto", value_name = "WHEN")]
  heading: Heading,

  // context related options
  /// Show NUM lines after each match.
  ///
  /// It conflicts with both the -C/--context flag.
  #[clap(
    short = 'A',
    long,
    default_value = "0",
    conflicts_with = "context",
    value_name = "NUM"
  )]
  after: u16,

  /// Show NUM lines before each match.
  ///
  /// It conflicts with both the -C/--context flag.
  #[clap(
    short = 'B',
    long,
    default_value = "0",
    conflicts_with = "context",
    value_name = "NUM"
  )]
  before: u16,

  /// Show NUM lines around each match.
  ///
  /// This is equivalent to providing both the
  /// -B/--before and -A/--after flags with the same value.
  /// It conflicts with both the -B/--before and -A/--after flags.
  #[clap(short = 'C', long, default_value = "0", value_name = "NUM")]
  context: u16,
}

impl RunArg {
  fn build_pattern(&self, lang: SgLang) -> Result<Pattern<SgLang>> {
    let pattern = if let Some(sel) = &self.selector {
      Pattern::contextual(&self.pattern, sel, lang)
    } else {
      Pattern::try_new(&self.pattern, lang)
    }
    .context(EC::ParsePattern)?;
    if let Some(strictness) = &self.strictness {
      Ok(pattern.with_strictness(strictness.0.clone()))
    } else {
      Ok(pattern)
    }
  }
}

// Every run will include Search or Replace
// Search or Replace by arguments `pattern` and `rewrite` passed from CLI
pub fn run_with_pattern(arg: RunArg) -> Result<()> {
  let context = if arg.context != 0 {
    (arg.context, arg.context)
  } else {
    (arg.before, arg.after)
  };
  if let Some(json) = arg.output.json {
    let printer = JSONPrinter::stdout(json).context(context);
    return run_pattern_with_printer(arg, printer);
  }
  let printer = ColoredPrinter::stdout(arg.output.color)
    .heading(arg.heading)
    .context(context);
  let interactive = arg.output.needs_interactive();
  if interactive {
    let from_stdin = arg.input.stdin;
    let printer = InteractivePrinter::new(printer, arg.output.update_all, from_stdin)?;
    run_pattern_with_printer(arg, printer)
  } else {
    run_pattern_with_printer(arg, printer)
  }
}

fn run_pattern_with_printer(arg: RunArg, printer: impl Printer + 'static) -> Result<()> {
  if arg.input.stdin {
    RunWithSpecificLang::new(arg, printer)?.run_std_in()
  } else if arg.lang.is_some() {
    RunWithSpecificLang::new(arg, printer)?.run_path()
  } else {
    let stats = arg.output.tracing.run_stats();
    RunWithInferredLang {
      arg,
      printer,
      stats,
    }
    .run_path()
  }
}

struct RunWithInferredLang<Printer> {
  arg: RunArg,
  printer: Printer,
  stats: RunStats,
}
impl<P: Printer> Worker for RunWithInferredLang<P> {
  type Item = (MatchUnit<Pattern<SgLang>>, SgLang);

  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    let rewrite = &self.arg.rewrite;
    let printer = &self.printer;
    printer.before_print()?;
    for (match_unit, lang) in items {
      let rewrite = rewrite
        .as_ref()
        .map(|s| Fixer::from_str(s, &lang))
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
    // TODO: print stats
    // let file_stats = &self.stats.file_stats;
    // eprintln!("Scanned: {}, Skipped: {}", file_stats.scanned(), file_stats.skipped());
    printer.after_print()?;
    if let Some(stats) = self.stats.print() {
      eprintln!("{}", stats);
    }
    Ok(())
  }
}

impl<P: Printer> PathWorker for RunWithInferredLang<P> {
  fn build_walk(&self) -> Result<WalkParallel> {
    self.arg.input.walk()
  }
  fn get_stats(&self) -> &FileStats {
    &self.stats.file_stats
  }

  fn produce_item(&self, path: &Path) -> Option<Vec<Self::Item>> {
    let lang = SgLang::from_path(path)?;
    let matcher = self.arg.build_pattern(lang).ok()?;
    // match sub region
    if let Some(sub_langs) = lang.injectable_sg_langs() {
      let matchers = sub_langs.filter_map(|l| {
        let pattern = self.arg.build_pattern(l).ok()?;
        Some((l, pattern))
      });
      filter_file_pattern(path, lang, Some(matcher), matchers)
    } else {
      filter_file_pattern(path, lang, Some(matcher), std::iter::empty())
    }
  }
}

struct RunWithSpecificLang<Printer> {
  arg: RunArg,
  printer: Printer,
  pattern: Pattern<SgLang>,
  rewrite: Option<Fixer<SgLang>>,
  stats: RunStats,
}

impl<Printer> RunWithSpecificLang<Printer> {
  fn new(arg: RunArg, printer: Printer) -> Result<Self> {
    let lang = arg.lang.ok_or(anyhow::anyhow!(EC::LanguageNotSpecified))?;
    let pattern = arg.build_pattern(lang)?;
    if let Some(format) = arg.debug_query {
      format.debug_query(&arg.pattern, &pattern, lang, arg.output.color);
    }
    let rewrite = if let Some(s) = &arg.rewrite {
      Some(Fixer::from_str(s, &lang).context(EC::ParsePattern)?)
    } else {
      None
    };
    let stats = arg.output.tracing.run_stats();
    Ok(Self {
      arg,
      printer,
      pattern,
      rewrite,
      stats,
    })
  }
}

impl<P: Printer> Worker for RunWithSpecificLang<P> {
  type Item = MatchUnit<Pattern<SgLang>>;

  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    let printer = &self.printer;
    printer.before_print()?;
    let mut has_matches = false;
    for match_unit in items {
      match_one_file(printer, &match_unit, &self.rewrite)?;
      has_matches = true;
    }
    printer.after_print()?;
    if let Some(stats) = self.stats.print() {
      eprintln!("{}", stats);
    }
    if !has_matches && self.pattern.has_error() {
      Err(anyhow::anyhow!(EC::PatternHasError))
    } else {
      Ok(())
    }
  }
}

impl<P: Printer> PathWorker for RunWithSpecificLang<P> {
  fn build_walk(&self) -> Result<WalkParallel> {
    let lang = self.arg.lang.expect("must present");
    Ok(self.arg.input.walk_lang(lang))
  }
  fn get_stats(&self) -> &FileStats {
    &self.stats.file_stats
  }
  fn produce_item(&self, path: &Path) -> Option<Vec<Self::Item>> {
    let arg = &self.arg;
    let pattern = self.pattern.clone();
    let lang = arg.lang.expect("must present");
    let path_lang = SgLang::from_path(path)?;
    let ret = if path_lang == lang {
      filter_file_pattern(path, lang, Some(pattern), std::iter::empty())?
    } else {
      filter_file_pattern(path, path_lang, None, std::iter::once((lang, pattern)))?
    };
    Some(ret.into_iter().map(|n| n.0).collect())
  }
}

impl<P: Printer> StdInWorker for RunWithSpecificLang<P> {
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
  rewrite: &Option<Fixer<SgLang>>,
) -> Result<()> {
  let MatchUnit {
    path,
    grep,
    matcher,
  } = match_unit;

  let matches = grep.root().find_all(matcher);
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
  use crate::print::ColorArg;
  use ast_grep_language::SupportLang;

  fn default_run_arg() -> RunArg {
    RunArg {
      pattern: String::new(),
      selector: None,
      rewrite: None,
      lang: None,
      heading: Heading::Never,
      debug_query: None,
      strictness: None,
      input: InputArgs {
        no_ignore: vec![],
        stdin: false,
        follow: false,
        paths: vec![PathBuf::from(".")],
        globs: vec![],
      },
      output: OutputArgs {
        color: ColorArg::Never,
        interactive: false,
        json: None,
        update_all: false,
        tracing: Default::default(),
      },
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
  fn test_run_with_strictness() {
    let arg = RunArg {
      pattern: "console.log".to_string(),
      strictness: Some(Strictness(MatchStrictness::Ast)),
      ..default_run_arg()
    };
    assert!(run_with_pattern(arg).is_ok())
  }

  #[test]
  fn test_run_with_specific_lang() {
    let arg = RunArg {
      pattern: "Some(result)".to_string(),
      lang: Some(SupportLang::Rust.into()),
      ..default_run_arg()
    };
    assert!(run_with_pattern(arg).is_ok())
  }
}
