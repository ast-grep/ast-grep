use anyhow::Result;
use std::borrow::Cow;
use std::fmt::Display;
use std::path::Path;

use ansi_term::{Color, Style};
use clap::arg_enum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::term::{self, DisplayStyle};
use similar::{ChangeTag, TextDiff};

use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::{Matcher, NodeMatch, Pattern};

pub use codespan_reporting::{files::SimpleFile, term::ColorArg};

use crate::languages::SupportLang;

pub struct ErrorReporter {
  writer: StandardStream,
  config: term::Config,
}

arg_enum! {
    #[derive(Debug)]
    pub enum ReportStyle {
        Rich,
        Medium,
        Short,
    }
}

impl ErrorReporter {
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

  pub fn print_rule<'a>(
    &self,
    matches: impl Iterator<Item = NodeMatch<'a, SupportLang>>,
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
        .with_message(&rule.message)
        .with_notes(rule.note.iter().cloned().collect())
        .with_labels(labels);
      term::emit(lock, config, &file, &diagnostic).unwrap();
    }
  }
}

#[cfg(not(target_os = "windows"))]
fn adjust_canonicalization<P: AsRef<Path>>(p: P) -> String {
  p.as_ref().display().to_string()
}

#[cfg(target_os = "windows")]
fn adjust_canonicalization<P: AsRef<Path>>(p: P) -> String {
  const VERBATIM_PREFIX: &str = r#"\\?\"#;
  let p = p.as_ref().display().to_string();
  if p.starts_with(VERBATIM_PREFIX) {
    p[VERBATIM_PREFIX.len()..].to_string()
  } else {
    p
  }
}

pub fn print_matches<'a>(
  matches: impl Iterator<Item = NodeMatch<'a, SupportLang>>,
  path: &Path,
  pattern: &impl Matcher<SupportLang>,
  rewrite: &Option<Pattern<SupportLang>>,
) -> Result<()> {
  let lock = std::io::stdout().lock(); // lock stdout to avoid interleaving output
                                       // dependencies on the system env, print different delimiters
  let filepath = adjust_canonicalization(std::fs::canonicalize(path)?);
  println!("{}", Color::Cyan.italic().paint(filepath));
  if let Some(rewrite) = rewrite {
    // TODO: actual matching happened in stdout lock, optimize it out
    for e in matches {
      let display = e.display_context(3);
      let old_str = format!(
        "{}{}{}\n",
        display.leading, display.matched, display.trailing
      );
      let new_str = format!(
        "{}{}{}\n",
        display.leading,
        e.replace(pattern, rewrite).unwrap().inserted_text,
        display.trailing
      );
      let base_line = display.start_line;
      print_diff(&old_str, &new_str, base_line);
    }
  } else {
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

fn print_diff(old: &str, new: &str, base_line: usize) {
  static THISTLE1: Color = Color::Fixed(225);
  static SEA_GREEN: Color = Color::Fixed(158);
  static RED: Color = Color::Fixed(161);
  static GREEN: Color = Color::Fixed(35);
  let diff = TextDiff::from_lines(old, new);
  let width = base_line.to_string().chars().count();
  for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
    if idx > 0 {
      println!("{:-^1$}", "-", 80);
    }
    for op in group {
      for change in diff.iter_inline_changes(op) {
        let (sign, s, bg) = match change.tag() {
          ChangeTag::Delete => (
            "-",
            Style::new().fg(RED).on(THISTLE1),
            Style::new().on(THISTLE1),
          ),
          ChangeTag::Insert => (
            "+",
            Style::new().fg(GREEN).on(SEA_GREEN),
            Style::new().on(SEA_GREEN),
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
            print!("{}", s.bold().paint(value));
          } else {
            print!("{}", bg.paint(value));
          }
        }
        if change.missing_newline() {
          println!();
        }
      }
    }
  }
}
