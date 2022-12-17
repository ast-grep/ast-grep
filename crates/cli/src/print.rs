use anyhow::Result;
use std::borrow::Cow;
use std::fmt::Display;
use std::path::Path;

use ansi_term::{Color, Style};
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::term::{self, DisplayStyle};
use similar::{ChangeTag, TextDiff};

use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::{Matcher, NodeMatch, Pattern};

pub use codespan_reporting::{files::SimpleFile, term::ColorArg};

use crate::languages::SupportLang;

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SupportLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

pub trait Printer {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SupportLang>,
  );
  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()>;
  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()>;
}

#[derive(Clone, ValueEnum)]
pub enum ReportStyle {
  Rich,
  Medium,
  Short,
}

pub struct ColoredPrinter {
  writer: StandardStream,
  config: term::Config,
}

impl ColoredPrinter {
  pub fn new(color: ColorChoice, style: ReportStyle) -> Self {
    let display_style = match style {
      ReportStyle::Rich => DisplayStyle::Rich,
      ReportStyle::Medium => DisplayStyle::Medium,
      ReportStyle::Short => DisplayStyle::Short,
    };
    Self {
      writer: StandardStream::stdout(color),
      config: term::Config {
        display_style,
        ..Default::default()
      },
    }
  }
}

impl Printer for ColoredPrinter {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SupportLang>,
  ) {
    let config = &self.config;
    let writer = &self.writer;
    let serverity = match rule.severity {
      Severity::Error => diagnostic::Severity::Error,
      Severity::Warning => diagnostic::Severity::Warning,
      Severity::Info => diagnostic::Severity::Note,
      Severity::Hint => diagnostic::Severity::Help,
    };
    let lock = &mut writer.lock();
    for m in matches {
      let range = m.range();
      let mut labels = vec![Label::primary((), range)];
      if let Some(secondary_nodes) = m.get_env().get_labels("secondary") {
        labels.extend(secondary_nodes.iter().map(|n| {
          let range = n.range();
          Label::secondary((), range)
        }));
      }
      let diagnostic = Diagnostic::new(serverity)
        .with_code(&rule.id)
        .with_message(rule.get_message(&m))
        .with_notes(rule.note.iter().cloned().collect())
        .with_labels(labels);
      term::emit(lock, config, &file, &diagnostic).unwrap();
    }
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    print_matches(matches, path)
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    print_diffs(diffs, path)
  }
}

#[cfg(not(target_os = "windows"))]
fn adjust_dir_separator(p: &Path) -> Cow<str> {
  p.to_string_lossy()
}

// change \ to / on windows
#[cfg(target_os = "windows")]
fn adjust_dir_separator(p: &Path) -> String {
  const VERBATIM_PREFIX: &str = r#"\\?\"#;
  let p = p.display().to_string();
  if p.starts_with(VERBATIM_PREFIX) {
    p[VERBATIM_PREFIX.len()..].to_string()
  } else {
    p
  }
}

fn print_prelude(path: &Path) -> std::io::StdoutLock {
  let lock = std::io::stdout().lock(); // lock stdout to avoid interleaving output
  let filepath = adjust_dir_separator(path);
  println!("{}", Color::Cyan.italic().paint(filepath));
  lock
}

fn print_matches<'a>(matches: Matches!('a), path: &Path) -> Result<()> {
  let lock = print_prelude(path);
  for e in matches {
    let display = e.display_context(0);
    let leading = display.leading;
    let trailing = display.trailing;
    let matched = display.matched;
    let highlighted = format!("{leading}{matched}{trailing}");
    let lines = highlighted.lines().count();
    let mut num = display.start_line;
    let width = (lines + display.start_line).to_string().chars().count();
    print!("{num:>width$}|"); // initial line num
    print_highlight(leading.lines(), Style::new().dimmed(), width, &mut num);
    print_highlight(matched.lines(), Style::new().bold(), width, &mut num);
    print_highlight(trailing.lines(), Style::new().dimmed(), width, &mut num);
    println!(); // end match new line
  }
  drop(lock);
  Ok(())
}

pub struct Diff<'n> {
  /// the matched node
  pub node_match: NodeMatch<'n, SupportLang>,
  /// string content for the replacement
  pub replacement: Cow<'n, str>,
}

impl<'n> Diff<'n> {
  pub fn generate(
    node_match: NodeMatch<'n, SupportLang>,
    matcher: &impl Matcher<SupportLang>,
    rewrite: &Pattern<SupportLang>,
  ) -> Self {
    let replacement = Cow::Owned(
      node_match
        .replace(matcher, rewrite)
        .expect("edit must exist")
        .inserted_text,
    );

    Self {
      node_match,
      replacement,
    }
  }
}

fn print_diffs<'a>(diffs: Diffs!('a), path: &Path) -> Result<()> {
  let lock = print_prelude(path);
  // TODO: actual matching happened in stdout lock, optimize it out
  for diff in diffs {
    let display = diff.node_match.display_context(3);
    let old_str = format!(
      "{}{}{}\n",
      display.leading, display.matched, display.trailing
    );
    let new_str = format!(
      "{}{}{}\n",
      display.leading, diff.replacement, display.trailing
    );
    let base_line = display.start_line;
    print_diff(&old_str, &new_str, base_line);
  }
  drop(lock);
  Ok(())
}

fn print_highlight<'a>(
  mut lines: impl Iterator<Item = &'a str>,
  style: Style,
  width: usize,
  num: &mut usize,
) {
  if let Some(line) = lines.next() {
    let line = style.paint(line);
    print!("{line}");
  }
  for line in lines {
    println!();
    *num += 1;
    let line = style.paint(line);
    print!("{num:>width$}|{line}");
  }
}

fn index_display(index: Option<usize>, style: Style, width: usize) -> impl Display {
  let index_str = match index {
    None => format!("{:width$}", ""),
    Some(idx) => format!("{:<width$}", idx),
  };
  style.paint(index_str)
}

pub fn print_diff(old: &str, new: &str, base_line: usize) {
  static THISTLE1: Color = Color::Fixed(225);
  static SEA_GREEN: Color = Color::Fixed(158);
  static RED: Color = Color::Fixed(161);
  static GREEN: Color = Color::Fixed(35);
  let diff = TextDiff::from_lines(old, new);
  let width = base_line.to_string().chars().count();
  for (idx, group) in diff.grouped_ops(5).iter().enumerate() {
    if idx > 0 {
      println!("{:-^1$}", "-", 80);
    }
    for op in group {
      for change in diff.iter_inline_changes(op) {
        let (sign, s, em) = match change.tag() {
          ChangeTag::Delete => ("-", Style::new().fg(RED), Style::new().fg(RED).on(THISTLE1)),
          ChangeTag::Insert => (
            "+",
            Style::new().fg(GREEN),
            Style::new().fg(GREEN).on(SEA_GREEN),
          ),
          ChangeTag::Equal => (" ", Style::new().dimmed(), Style::new()),
        };
        print!(
          "{}{}|{}",
          index_display(change.old_index().map(|i| i + base_line), s, width + 1),
          index_display(change.new_index().map(|i| i + base_line), s, width),
          s.paint(sign),
        );
        for (emphasized, value) in change.iter_strings_lossy() {
          if emphasized {
            print!("{}", em.bold().paint(value));
          } else {
            print!("{}", s.paint(value));
          }
        }
        if change.missing_newline() {
          println!();
        }
      }
    }
  }
}

pub struct JSONPrinter;
impl Printer for JSONPrinter {
  fn print_rule<'a>(
    &self,
    _matches: Matches!('a),
    _file: SimpleFile<Cow<str>, &String>,
    _rule: &RuleConfig<SupportLang>,
  ) {
    todo!("")
  }

  fn print_matches<'a>(&self, _matches: Matches!('a), _path: &Path) -> Result<()> {
    todo!("")
  }

  fn print_diffs<'a>(&self, _diffs: Diffs!('a), _path: &Path) -> Result<()> {
    todo!("")
  }
}
