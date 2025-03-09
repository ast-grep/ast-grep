mod args;
mod debug_query;
mod error_context;
mod inspect;
mod print_diff;
mod rule_overwrite;
mod worker;

pub use args::{ContextArgs, InputArgs, OutputArgs, OverwriteArgs};
pub use debug_query::DebugFormat;
pub use error_context::{exit_with_error, ErrorContext};
pub use inspect::{FileTrace, Granularity, RuleTrace, RunTrace, ScanTrace};
pub use print_diff::DiffStyles;
pub use rule_overwrite::RuleOverwrite;
pub use worker::{Items, PathWorker, StdInWorker, Worker};

use crate::lang::SgLang;

use anyhow::{anyhow, Context, Result};
use crossterm::{
  cursor::MoveTo,
  event::{self, Event, KeyCode},
  execute,
  terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
  terminal::{Clear, ClearType},
};
use smallvec::{smallvec, SmallVec};

use ast_grep_config::RuleCollection;
use ast_grep_core::Pattern;
use ast_grep_core::{Matcher, StrDoc};
use ast_grep_language::Language;

use std::fs::read_to_string;
use std::io::stdout;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

type AstGrep = ast_grep_core::AstGrep<StrDoc<SgLang>>;

fn read_char() -> Result<char> {
  loop {
    if let Event::Key(evt) = event::read()? {
      match evt.code {
        KeyCode::Enter => break Ok('\n'),
        KeyCode::Char(c) => break Ok(c),
        _ => (),
      }
    }
  }
}

/// Prompts for user input on STDOUT
fn prompt_reply_stdout(prompt: &str) -> Result<char> {
  let mut stdout = std::io::stdout();
  write!(stdout, "{}", prompt)?;
  stdout.flush()?;
  terminal::enable_raw_mode()?;
  let ret = read_char();
  terminal::disable_raw_mode()?;
  ret
}

// clear screen
fn clear() -> Result<()> {
  execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
  Ok(())
  // https://github.com/console-rs/console/blob/be1c2879536c90ffc2b54938b5964084f5fef67d/src/common_term.rs#L56
  // print!("\r\x1b[2J\r\x1b[H");
}

pub fn run_in_alternate_screen<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
  execute!(stdout(), EnterAlternateScreen)?;
  clear()?;
  let ret = f();
  execute!(stdout(), LeaveAlternateScreen)?;
  ret
}

pub fn prompt(prompt_text: &str, letters: &str, default: Option<char>) -> Result<char> {
  loop {
    let input = prompt_reply_stdout(prompt_text)?;
    if let Some(default) = default {
      if input == '\n' {
        return Ok(default);
      }
    }
    if letters.contains(input) {
      return Ok(input);
    }
    eprintln!("Unrecognized command, try again?")
  }
}

fn read_file(path: &Path) -> Result<String> {
  let file_content =
    read_to_string(path).with_context(|| format!("Cannot read file {}", path.to_string_lossy()))?;
  // skip large files or empty file
  if file_too_large(&file_content) {
    Err(anyhow!("File is too large"))
  } else if file_content.is_empty() {
    Err(anyhow!("File is empty"))
  } else {
    Ok(file_content)
  }
}

fn collect_file_stats(
  path: &Path,
  lang: SgLang,
  configs: &RuleCollection<SgLang>,
  rule_stats: &ScanTrace,
) -> Result<()> {
  let rules = configs.get_rule_from_lang(path, lang);
  rule_stats.print_file(path, lang, &rules)?;
  Ok(())
}

pub fn filter_file_rule(
  path: &Path,
  configs: &RuleCollection<SgLang>,
  trace: &ScanTrace,
) -> Result<SmallVec<[AstGrep; 1]>> {
  let Some(lang) = SgLang::from_path(path) else {
    return Ok(smallvec![]);
  };
  let file_content = read_file(path)?;
  let grep = lang.ast_grep(file_content);
  collect_file_stats(path, lang, configs, trace)?;
  let mut ret = smallvec![grep.clone()];
  if let Some(injected) = lang.injectable_sg_langs() {
    let docs = grep.inner.get_injections(|s| SgLang::from_str(s).ok());
    let inj = injected.filter_map(|l| {
      let doc = docs.iter().find(|d| *d.lang() == l)?;
      let grep = AstGrep { inner: doc.clone() };
      collect_file_stats(path, l, configs, trace).ok()?;
      Some(grep)
    });
    ret.extend(inj)
  }
  Ok(ret)
}

// sub_matchers are the injected languages
// e.g. js/css in html
pub fn filter_file_pattern<'a>(
  path: &Path,
  lang: SgLang,
  root_matcher: Option<&'a Pattern<SgLang>>,
  sub_matchers: &'a [(SgLang, Pattern<SgLang>)],
) -> Result<SmallVec<[MatchUnit<&'a Pattern<SgLang>>; 1]>> {
  let file_content = read_file(path)?;
  let grep = lang.ast_grep(&file_content);
  let do_match = |ast_grep: AstGrep, matcher: &'a Pattern<SgLang>| {
    let fixed = matcher.fixed_string();
    if !fixed.is_empty() && !file_content.contains(&*fixed) {
      return None;
    }
    Some(MatchUnit {
      grep: ast_grep,
      path: path.to_path_buf(),
      matcher,
    })
  };
  let mut ret = smallvec![];
  if let Some(matcher) = root_matcher {
    ret.extend(do_match(grep.clone(), matcher));
  }
  let injections = grep.inner.get_injections(|s| SgLang::from_str(s).ok());
  let sub_units = injections.into_iter().filter_map(|inner| {
    let (_, matcher) = sub_matchers.iter().find(|i| *inner.lang() == i.0)?;
    let injected = AstGrep { inner };
    do_match(injected, matcher)
  });
  ret.extend(sub_units);
  Ok(ret)
}

const MAX_FILE_SIZE: usize = 3_000_000;
const MAX_LINE_COUNT: usize = 200_000;

// skip files that are too large in size AND have too many lines
fn file_too_large(file_content: &str) -> bool {
  // the && operator is intentional here to include more files
  file_content.len() > MAX_FILE_SIZE && file_content.lines().count() > MAX_LINE_COUNT
}

/// A single atomic unit where matches happen.
/// It contains the file path, sg instance and matcher.
/// An analogy to compilation unit in C programming language.
pub struct MatchUnit<M: Matcher<SgLang>> {
  pub path: PathBuf,
  pub grep: AstGrep,
  pub matcher: M,
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_language::SupportLang;

  #[test]
  fn test_html_embedding() {
    let root =
      SgLang::Builtin(SupportLang::Html).ast_grep("<script lang=typescript>alert(123)</script>");
    let docs = root.inner.get_injections(|s| SgLang::from_str(s).ok());
    assert_eq!(docs.len(), 1);
    let script = docs[0].root().child(0).expect("should exist");
    assert_eq!(script.kind(), "expression_statement");
  }

  #[test]
  fn test_html_embedding_lang_not_found() {
    let root = SgLang::Builtin(SupportLang::Html).ast_grep("<script lang=xxx>alert(123)</script>");
    let docs = root.inner.get_injections(|s| SgLang::from_str(s).ok());
    assert_eq!(docs.len(), 0);
  }
}
