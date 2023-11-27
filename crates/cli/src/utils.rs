use crate::lang::SgLang;
use crate::print::{ColorArg, JsonStyle};

use anyhow::{anyhow, Context, Result};
use clap::{Args, ValueEnum};
use crossterm::{
  event::{self, Event, KeyCode},
  execute,
  terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ignore::{DirEntry, WalkBuilder, WalkParallel, WalkState};
use serde::{Deserialize, Serialize};

use ast_grep_core::Pattern;
use ast_grep_core::{AstGrep, Matcher, StrDoc};
use ast_grep_language::Language;

use std::env;
use std::fs::read_to_string;
use std::io::stdout;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

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

// https://github.com/console-rs/console/blob/be1c2879536c90ffc2b54938b5964084f5fef67d/src/common_term.rs#L56
// clear screen
fn clear() {
  print!("\r\x1b[2J\r\x1b[H");
}

pub fn run_in_alternate_screen<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
  execute!(stdout(), EnterAlternateScreen)?;
  clear();
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
    println!("Unrecognized command, try again?")
  }
}

/// A trait to abstract how ast-grep discovers, parses and processes files.
///
/// It follows multiple-producer-single-consumer pattern.
/// ast-grep discovers files in parallel by `build_walk`.
/// Then every file is parsed and filtered in `produce_item`.
/// Finally, `produce_item` will send `Item` to the consumer thread.
pub trait Worker: Sync {
  /// The item to send between producer/consumer threads.
  /// It is usually parsed tree-sitter Root with optional data.
  type Item: Send;
  /// WalkParallel will determine what files will be processed.
  fn build_walk(&self) -> WalkParallel;
  /// Parse and find_match can be done in `produce_item`.
  fn produce_item(&self, path: &Path) -> Option<Self::Item>;
  /// `consume_items` will run in a separate single thread.
  /// printing matches or error reporting can happen here.
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()>;
}

pub trait StdInWorker: Worker {
  fn parse_stdin(&self, src: String) -> Option<Self::Item>;
}

pub struct Items<T>(mpsc::Receiver<T>);
impl<T> Iterator for Items<T> {
  type Item = T;
  fn next(&mut self) -> Option<Self::Item> {
    if let Ok(match_result) = self.0.recv() {
      Some(match_result)
    } else {
      None
    }
  }
}
impl<T> Items<T> {
  pub fn once(t: T) -> Result<Self> {
    let (tx, rx) = mpsc::channel();
    // use write to avoid send/sync trait bound
    match tx.send(t) {
      Ok(_) => (),
      Err(e) => return Err(anyhow!(e.to_string())),
    };
    Ok(Items(rx))
  }
}

fn filter_result(result: Result<DirEntry, ignore::Error>) -> Option<PathBuf> {
  let entry = match result {
    Ok(entry) => entry,
    Err(err) => {
      eprintln!("ERROR: {}", err);
      return None;
    }
  };
  entry.file_type()?.is_file().then(|| entry.into_path())
}

pub fn run_std_in<MW: StdInWorker>(worker: MW) -> Result<()> {
  let source = std::io::read_to_string(std::io::stdin())?;
  if let Some(item) = worker.parse_stdin(source) {
    worker.consume_items(Items::once(item)?)
  } else {
    Ok(())
  }
}

pub fn run_worker<MW: Worker>(worker: MW) -> Result<()> {
  let producer = |path: PathBuf| worker.produce_item(&path);
  let (tx, rx) = mpsc::channel();
  let walker = worker.build_walk();
  walker.run(|| {
    let tx = tx.clone();
    Box::new(move |result| {
      let maybe_result = filter_result(result).and_then(producer);
      let result = match maybe_result {
        Some(ret) => ret,
        None => return WalkState::Continue,
      };
      match tx.send(result) {
        Ok(_) => WalkState::Continue,
        Err(_) => WalkState::Quit,
      }
    })
  });
  // drop the last sender to stop rx awaiting message
  drop(tx);
  worker.consume_items(Items(rx))
}

fn read_file(path: &Path) -> Option<String> {
  let file_content = read_to_string(path)
    .with_context(|| format!("Cannot read file {}", path.to_string_lossy()))
    .map_err(|err| eprintln!("{err}"))
    .ok()?;
  // skip large files or empty file
  if file_too_large(&file_content) || file_content.is_empty() {
    // TODO add output
    None
  } else {
    Some(file_content)
  }
}

pub fn filter_file_interactive<M: Matcher<SgLang>>(
  path: &Path,
  lang: SgLang,
  matcher: M,
) -> Option<MatchUnit<M>> {
  let file_content = read_file(path)?;
  let grep = lang.ast_grep(file_content);
  let has_match = grep.root().find(&matcher).is_some();
  has_match.then(|| MatchUnit {
    grep,
    path: path.to_path_buf(),
    matcher,
  })
}

pub fn filter_file_pattern(
  path: &Path,
  lang: SgLang,
  matcher: Pattern<StrDoc<SgLang>>,
) -> Option<MatchUnit<Pattern<StrDoc<SgLang>>>> {
  let file_content = read_file(path)?;
  let fixed = matcher.fixed_string();
  if !fixed.is_empty() && !file_content.contains(&*fixed) {
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

const MAX_FILE_SIZE: usize = 3_000_000;
const MAX_LINE_COUNT: usize = 200_000;

fn file_too_large(file_content: &String) -> bool {
  file_content.len() > MAX_FILE_SIZE && file_content.lines().count() > MAX_LINE_COUNT
}

/// A single atomic unit where matches happen.
/// It contains the file path, sg instance and matcher.
/// An analogy to compilation unit in C programming language.
pub struct MatchUnit<M: Matcher<SgLang>> {
  pub path: PathBuf,
  pub grep: AstGrep<StrDoc<SgLang>>,
  pub matcher: M,
}

#[inline]
fn is_from_stdin() -> bool {
  // disable stdin if tty env presents or is_atty == true
  // env is used for testing purpose only
  env::var_os("AST_GREP_NO_STDIN").is_none() && !atty::is(atty::Stream::Stdin)
}

/// input related options
#[derive(Args)]
pub struct InputArgs {
  /// The paths to search. You can provide multiple paths separated by spaces.
  #[clap(value_parser, default_value = ".")]
  pub paths: Vec<PathBuf>,

  /// Do not respect hidden file system or ignore files (.gitignore, .ignore, etc.).
  ///
  /// You can suppress multiple ignore files by passing `no-ignore` multiple times.
  #[clap(long, action = clap::ArgAction::Append, value_name = "FILE_TYPE")]
  pub no_ignore: Vec<IgnoreFile>,

  /// Enable search code from StdIn.
  ///
  /// Use this if you need to take code stream from standard input.
  /// If the environment variable `AST_GREP_NO_STDIN` exist, ast-grep will disable StdIn mode.
  #[clap(long)]
  pub stdin: bool,
}

impl InputArgs {
  pub fn is_stdin(&self) -> bool {
    self.stdin && is_from_stdin()
  }

  pub fn walk(&self) -> WalkParallel {
    let threads = num_cpus::get().min(12);
    NoIgnore::disregard(&self.no_ignore)
      .walk(&self.paths)
      .threads(threads)
      .build_parallel()
  }

  pub fn walk_lang(&self, lang: SgLang) -> WalkParallel {
    let threads = num_cpus::get().min(12);
    NoIgnore::disregard(&self.no_ignore)
      .walk(&self.paths)
      .threads(threads)
      .types(lang.file_types())
      .build_parallel()
  }
}

/// output related options
#[derive(Args)]
pub struct OutputArgs {
  /// Start interactive edit session.
  ///
  /// You can confirm the code change and apply it to files selectively,
  /// or you can open text editor to tweak the matched code.
  /// Note that code rewrite only happens inside a session.
  #[clap(short, long)]
  pub interactive: bool,

  /// Apply all rewrite without confirmation if true.
  #[clap(short = 'U', long)]
  pub update_all: bool,

  /// Output matches in structured JSON .
  ///
  /// If this flag is set, ast-grep will output matches in JSON format.
  /// You can pass optional value to this flag by using `--json=<style>` syntax
  /// to further control how JSON object is formatted and printed. ast-grep will `pretty`-print JSON if no value is passed.
  /// Note, the json flag must use `=` to specify its value.
  /// It conflicts with interactive.
  #[clap(
      long,
      conflicts_with = "interactive",
      value_name="style",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pretty"
  )]
  pub json: Option<JsonStyle>,

  /// Controls output color.
  ///
  /// This flag controls when to use colors. The default setting is 'auto', which
  /// means ast-grep will try to guess when to use colors. If ast-grep is
  /// printing to a terminal, then it will use colors, but if it is redirected to a
  /// file or a pipe, then it will suppress color output. ast-grep will also suppress
  /// color output in some other circumstances. For example, no color will be used
  /// if the TERM environment variable is not set or set to 'dumb'.
  #[clap(long, default_value = "auto", value_name = "WHEN")]
  pub color: ColorArg,
}

impl OutputArgs {
  // either explicit interactive or implicit update_all
  pub fn needs_interactive(&self) -> bool {
    self.interactive || self.update_all
  }
}

/// File types to ignore, this is mostly the same as ripgrep.
#[derive(Clone, Copy, Deserialize, Serialize, ValueEnum)]
pub enum IgnoreFile {
  /// Search hidden files and directories. By default, hidden files and directories are skipped.
  Hidden,
  /// Don't respect .ignore files.
  /// This does *not* affect whether ast-grep will ignore files and directories whose names begin with a dot.
  /// For that, use --no-ignore hidden.
  Dot,
  /// Don't respect ignore files that are manually configured for the repository such as git's '.git/info/exclude'.
  Exclude,
  /// Don't respect ignore files that come from "global" sources such as git's
  /// `core.excludesFile` configuration option (which defaults to `$HOME/.config/git/ignore`).
  Global,
  /// Don't respect ignore files (.gitignore, .ignore, etc.) in parent directories.
  Parent,
  /// Don't respect version control ignore files (.gitignore, etc.).
  /// This implies --no-ignore parent for VCS files.
  /// Note that .ignore files will continue to be respected.
  Vcs,
}

#[derive(Default)]
pub struct NoIgnore {
  disregard_hidden: bool,
  disregard_parent: bool,
  disregard_dot: bool,
  disregard_vcs: bool,
  disregard_global: bool,
  disregard_exclude: bool,
}

impl NoIgnore {
  pub fn disregard(ignores: &Vec<IgnoreFile>) -> Self {
    let mut ret = NoIgnore::default();
    use IgnoreFile::*;
    for ignore in ignores {
      match ignore {
        Hidden => ret.disregard_hidden = true,
        Dot => ret.disregard_dot = true,
        Exclude => ret.disregard_exclude = true,
        Global => ret.disregard_global = true,
        Parent => ret.disregard_parent = true,
        Vcs => ret.disregard_vcs = true,
      }
    }
    ret
  }

  pub fn walk(&self, path: &[PathBuf]) -> WalkBuilder {
    let mut paths = path.iter();
    let mut builder = WalkBuilder::new(paths.next().expect("non empty"));
    for path in paths {
      builder.add(path);
    }
    builder
      .hidden(!self.disregard_hidden)
      .parents(!self.disregard_parent)
      .ignore(!self.disregard_dot)
      .git_global(!self.disregard_vcs && !self.disregard_global)
      .git_ignore(!self.disregard_vcs)
      .git_exclude(!self.disregard_vcs && !self.disregard_exclude);
    builder
  }
}
