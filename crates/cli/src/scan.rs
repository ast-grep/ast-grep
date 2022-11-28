use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;
use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Matcher, Pattern};
use clap::{Args, Parser};
use ignore::{DirEntry, WalkBuilder, WalkParallel, WalkState};

use crate::config::find_config;
use crate::error::ErrorContext as EC;
use crate::interaction;
use crate::languages::{file_types, SupportLang};
use crate::print::{print_diffs, print_matches, ColorArg, ErrorReporter, ReportStyle, SimpleFile};

#[derive(Parser)]
pub struct RunArg {
  /// AST pattern to match
  #[clap(short, long)]
  pattern: String,

  /// String to replace the matched AST node
  #[clap(short, long)]
  rewrite: Option<String>,

  /// Print query pattern's tree-sitter AST
  #[clap(long)]
  debug_query: bool,

  /// The language of the pattern query
  #[clap(short, long)]
  lang: SupportLang,

  /// Start interactive edit session. Code rewrite only happens inside a session.
  #[clap(short, long)]
  interactive: bool,

  /// The path whose descendent files are to be explored.
  #[clap(value_parser, default_value = ".")]
  path: PathBuf,

  /// Apply all rewrite without confirmation if true.
  #[clap(long, requires = "rewrite")]
  accept_all: bool,

  /// Include hidden files in search
  #[clap(long)]
  hidden: bool,
}

#[derive(Args)]
pub struct ScanArg {
  /// Path to ast-grep config, either YAML or folder of YAMLs
  #[clap(short, long)]
  config: Option<PathBuf>,

  /// Include hidden files in search
  #[clap(long)]
  hidden: bool,

  /// Start interactive edit session. Code rewrite only happens inside a session.
  #[clap(short, long)]
  interactive: bool,

  /// Controls output color.
  #[clap(long, default_value = "auto")]
  color: ColorArg,

  #[clap(long, default_value = "rich")]
  report_style: ReportStyle,

  /// Apply all rewrite without confirmation if true.
  #[clap(long, requires = "rewrite")]
  accept_all: bool,

  /// The path whose descendent files are to be explored.
  #[clap(value_parser, default_value = ".")]
  path: PathBuf,
}

static ACCEPT_ALL: AtomicBool = AtomicBool::new(false);

// Every run will include Search or Replace
// Search or Replace by arguments `pattern` and `rewrite` passed from CLI
pub fn run_with_pattern(args: RunArg) -> Result<()> {
  let pattern = args.pattern;
  let threads = num_cpus::get().min(12);
  let lang = args.lang;
  let pattern = Pattern::new(&pattern, lang);
  if args.debug_query {
    println!("Pattern TreeSitter {:?}", pattern);
  }
  let walker = WalkBuilder::new(&args.path)
    .hidden(args.hidden)
    .threads(threads)
    .types(file_types(&lang))
    .build_parallel();
  let rewrite = args.rewrite.map(|s| Pattern::new(s.as_ref(), lang));
  let interactive = args.interactive || args.accept_all;
  if !interactive {
    run_walker(walker, |path| {
      match_one_file(path, lang, &pattern, &rewrite)
    })
  } else {
    ACCEPT_ALL.store(args.accept_all, Ordering::SeqCst);
    run_walker_interactive(
      walker,
      |path| filter_file_interactive(path, lang, &pattern),
      |(grep, path)| run_one_interaction(&path, &grep, &pattern, &rewrite),
    )
  }
}

pub fn run_with_config(args: ScanArg) -> Result<()> {
  let configs = find_config(args.config)?;
  let threads = num_cpus::get().min(12);
  let walker = WalkBuilder::new(&args.path)
    .hidden(args.hidden)
    .threads(threads)
    .build_parallel();
  let reporter = ErrorReporter::new(args.color.into(), args.report_style);
  let interactive = args.interactive || args.accept_all;
  if !interactive {
    run_walker(walker, |path| {
      for config in configs.for_path(path) {
        let lang = config.language;
        match_rule_on_file(path, lang, config, &reporter)?;
      }
      Ok(())
    })
  } else {
    ACCEPT_ALL.store(args.accept_all, Ordering::SeqCst);
    run_walker_interactive(
      walker,
      |path| {
        for config in configs.for_path(path) {
          let lang = config.language;
          let matcher = config.get_matcher();
          let ret = filter_file_interactive(path, lang, &matcher);
          if ret.is_some() {
            return ret;
          }
        }
        None
      },
      |(grep, path)| {
        for config in configs.for_path(&path) {
          let matcher = config.get_matcher();
          let fixer = config.get_fixer();
          run_one_interaction(&path, &grep, matcher, &fixer)?;
        }
        Ok(())
      },
    )
  }
}

const EDIT_PROMPT: &str = "Accept change? (Yes[y], No[n], Accept All[a], Quit[q], Edit[e])";
const VIEW_PROMPT: &str = "Next[enter], Quit[q]";

fn run_one_interaction<M: Matcher<SupportLang>>(
  path: &PathBuf,
  grep: &AstGrep<SupportLang>,
  matcher: M,
  rewrite: &Option<Pattern<SupportLang>>,
) -> Result<()> {
  if let Some(rewrite) = rewrite {
    interaction::run_in_alternate_screen(|| {
      print_diffs_and_prompt_action(path, grep, matcher, rewrite)
    })
  } else {
    interaction::run_in_alternate_screen(|| print_matches_and_confirm_next(path, grep, matcher))
  }
}

fn print_diffs_and_prompt_action<M: Matcher<SupportLang>>(
  path: &PathBuf,
  grep: &AstGrep<SupportLang>,
  matcher: M,
  rewrite: &Pattern<SupportLang>,
) -> Result<()> {
  let rewrite_action = || {
    let new_content = apply_rewrite(grep, &matcher, rewrite);
    std::fs::write(path, new_content).with_context(|| EC::WriteFile(path.clone()))?;
    Ok(())
  };
  if ACCEPT_ALL.load(Ordering::SeqCst) {
    return rewrite_action();
  }
  let mut matches = grep.root().find_all(&matcher).peekable();
  let first_match = match matches.peek() {
    Some(n) => n.start_pos().0,
    None => return Ok(()),
  };
  print_diffs(matches, path, &matcher, rewrite)?;
  let response =
    interaction::prompt(EDIT_PROMPT, "ynaqe", Some('n')).expect("Error happened during prompt");
  match response {
    'y' => rewrite_action(),
    'a' => {
      ACCEPT_ALL.store(true, Ordering::SeqCst);
      rewrite_action()
    }
    'n' => Ok(()),
    'e' => interaction::open_in_editor(path, first_match),
    'q' => Err(anyhow::anyhow!("Exit interactive editing")),
    _ => Ok(()),
  }
}

fn print_matches_and_confirm_next<M: Matcher<SupportLang>>(
  path: &Path,
  grep: &AstGrep<SupportLang>,
  matcher: M,
) -> Result<()> {
  let matches = grep.root().find_all(&matcher);
  print_matches(matches, path)?;
  let resp = interaction::prompt(VIEW_PROMPT, "q", Some('\n')).expect("cannot fail");
  if resp == 'q' {
    Err(anyhow::anyhow!("Exit interactive editing"))
  } else {
    Ok(())
  }
}

fn apply_rewrite<M: Matcher<SupportLang>>(
  grep: &AstGrep<SupportLang>,
  matcher: M,
  rewrite: &Pattern<SupportLang>,
) -> String {
  let root = grep.root();
  let edits = root.replace_all(matcher, rewrite);
  let mut new_content = String::new();
  let mut start = 0;
  for edit in edits {
    new_content.push_str(&grep.source()[start..edit.position]);
    new_content.push_str(&edit.inserted_text);
    start = edit.position + edit.deleted_length;
  }
  // add trailing statements
  new_content.push_str(&grep.source()[start..]);
  new_content
}

fn filter_file(entry: DirEntry) -> Option<DirEntry> {
  entry.file_type()?.is_file().then_some(entry)
}

fn run_walker(walker: WalkParallel, f: impl Fn(&Path) -> Result<()> + Sync) -> Result<()> {
  interaction::run_walker(walker, |entry| {
    if let Some(e) = filter_file(entry) {
      f(e.path())?;
    }
    Ok(WalkState::Continue)
  });
  Ok(())
}

fn run_walker_interactive<T: Send>(
  walker: WalkParallel,
  producer: impl Fn(&Path) -> Option<T> + Sync,
  consumer: impl Fn(T) -> Result<()> + Send,
) -> Result<()> {
  interaction::run_walker_interactive(
    walker,
    |entry| producer(filter_file(entry)?.path()),
    consumer,
  )
}

fn match_rule_on_file(
  path: &Path,
  lang: SupportLang,
  rule: &RuleConfig<SupportLang>,
  reporter: &ErrorReporter,
) -> Result<()> {
  let matcher = rule.get_matcher();
  let file_content = read_to_string(path)?;
  let grep = lang.ast_grep(&file_content);
  let mut matches = grep.root().find_all(matcher).peekable();
  if matches.peek().is_none() {
    return Ok(());
  }
  let file = SimpleFile::new(path.to_string_lossy(), &file_content);
  reporter.print_rule(matches, file, rule);
  Ok(())
}

fn match_one_file(
  path: &Path,
  lang: SupportLang,
  pattern: &impl Matcher<SupportLang>,
  rewrite: &Option<Pattern<SupportLang>>,
) -> Result<()> {
  let file_content = read_to_string(path)?;
  let grep = lang.ast_grep(file_content);
  let mut matches = grep.root().find_all(pattern).peekable();
  if matches.peek().is_none() {
    return Ok(());
  }
  if let Some(rewrite) = rewrite {
    print_diffs(matches, path, pattern, rewrite)
  } else {
    print_matches(matches, path)
  }
}

fn filter_file_interactive(
  path: &Path,
  lang: SupportLang,
  pattern: &impl Matcher<SupportLang>,
) -> Option<(AstGrep<SupportLang>, PathBuf)> {
  let file_content = read_to_string(path)
    .with_context(|| format!("Cannot read file {}", path.to_string_lossy()))
    .map_err(|err| eprintln!("{err}"))
    .ok()?;
  let grep = lang.ast_grep(file_content);
  let has_match = grep.root().find(pattern).is_some();
  has_match.then_some((grep, path.to_path_buf()))
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::from_yaml_string;

  fn make_rule(rule: &str) -> RuleConfig<SupportLang> {
    from_yaml_string(&format!(
      r"
id: test
message: test rule
severity: info
language: TypeScript
{rule}"
    ))
    .unwrap()
    .pop()
    .unwrap()
  }

  #[test]
  fn test_apply_rewrite() {
    let root = AstGrep::new("let a = () => c++", SupportLang::TypeScript);
    let config = make_rule(
      r"
rule:
  all:
    - pattern: $B
    - any:
        - pattern: $A++
fix: ($B, lifecycle.update(['$A']))",
    );
    let ret = apply_rewrite(&root, config.get_matcher(), &config.get_fixer().unwrap());
    assert_eq!(ret, "let a = () => (c++, lifecycle.update(['c']))");
  }

  #[test]
  fn test_rewrite_nested() {
    let root = SupportLang::TypeScript.ast_grep("Some(Some(1))");
    let ret = apply_rewrite(
      &root,
      "Some($A)",
      &Pattern::new("$A", SupportLang::TypeScript),
    );
    assert_eq!("Some(1)", ret);
  }
}
