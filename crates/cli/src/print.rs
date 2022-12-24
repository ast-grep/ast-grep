use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::{Matcher, MetaVariable, Node, NodeMatch, Pattern};
use ast_grep_language::SupportLang;
use std::collections::HashMap;

use ansi_term::{Color, Style};
use anyhow::Result;
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::term::{self, DisplayStyle};
pub use codespan_reporting::{files::SimpleFile, term::ColorArg};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

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
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    rule: &RuleConfig<SupportLang>,
  ) -> Result<()>;
  fn before_print(&self) {}
  fn after_print(&self) {}
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
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    _rule: &RuleConfig<SupportLang>,
  ) -> Result<()> {
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

fn print_prelude(path: &Path) {
  let filepath = adjust_dir_separator(path);
  println!("{}", Color::Cyan.italic().paint(filepath));
}

fn print_matches<'a>(matches: Matches!('a), path: &Path) -> Result<()> {
  print_prelude(path);
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
  print_prelude(path);
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Position {
  line: usize,
  column: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Range {
  /// inclusive start, exclusive end
  byte_offset: std::ops::Range<usize>,
  start: Position,
  end: Position,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelJSON<'a> {
  text: &'a str,
  range: Range,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchNode<'a> {
  text: Cow<'a, str>,
  range: Range,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchJSON<'a> {
  text: Cow<'a, str>,
  range: Range,
  file: Cow<'a, str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  replacement: Option<Cow<'a, str>>,
  language: SupportLang,
  #[serde(skip_serializing_if = "Option::is_none")]
  meta_variables: Option<MetaVariables<'a>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaVariables<'a> {
  single: HashMap<String, MatchNode<'a>>,
  multi: HashMap<String, Vec<MatchNode<'a>>>,
}
fn from_env<'a>(nm: &NodeMatch<'a, SupportLang>) -> Option<MetaVariables<'a>> {
  let env = nm.get_env();
  let mut vars = env.get_matched_variables().peekable();
  vars.peek()?;
  let mut single = HashMap::new();
  let mut multi = HashMap::new();
  for var in vars {
    use MetaVariable as MV;
    match var {
      MV::Named(n) => {
        let node = env.get_match(&n).expect("must exist!");
        single.insert(
          n,
          MatchNode {
            text: node.text(),
            range: get_range(node),
          },
        );
      }
      MV::NamedEllipsis(n) => {
        let nodes = env.get_multiple_matches(&n);
        multi.insert(
          n,
          nodes
            .into_iter()
            .map(|node| MatchNode {
              text: node.text(),
              range: get_range(&node),
            })
            .collect(),
        );
      }
      _ => continue,
    }
  }
  Some(MetaVariables { single, multi })
}

fn get_range(n: &Node<'_, SupportLang>) -> Range {
  let start_pos = n.start_pos();
  let end_pos = n.end_pos();
  Range {
    byte_offset: n.range(),
    start: Position {
      line: start_pos.0,
      column: start_pos.1,
    },
    end: Position {
      line: end_pos.0,
      column: end_pos.1,
    },
  }
}

impl<'a> MatchJSON<'a> {
  fn new(nm: NodeMatch<'a, SupportLang>, path: &'a str) -> Self {
    MatchJSON {
      file: Cow::Borrowed(path),
      text: nm.text(),
      language: *nm.lang(),
      replacement: None,
      range: get_range(&nm),
      meta_variables: from_env(&nm),
    }
  }
}
fn get_labels<'a>(nm: &NodeMatch<'a, SupportLang>) -> Option<Vec<MatchNode<'a>>> {
  let env = nm.get_env();
  let labels = env.get_labels("secondary")?;
  Some(
    labels
      .iter()
      .map(|l| MatchNode {
        text: l.text(),
        range: get_range(l),
      })
      .collect(),
  )
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleMatchJSON<'a> {
  #[serde(flatten)]
  matched: MatchJSON<'a>,
  rule_id: &'a str,
  severity: Severity,
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  labels: Option<Vec<MatchNode<'a>>>,
}
impl<'a> RuleMatchJSON<'a> {
  fn new(nm: NodeMatch<'a, SupportLang>, path: &'a str, rule: &'a RuleConfig<SupportLang>) -> Self {
    let message = rule.get_message(&nm);
    let labels = get_labels(&nm);
    let matched = MatchJSON::new(nm, path);
    Self {
      matched,
      rule_id: &rule.id,
      severity: rule.severity.clone(),
      message,
      labels,
    }
  }
}

// store atomic bool to indicate if any matches happened
pub struct JSONPrinter(AtomicBool);
impl JSONPrinter {
  pub fn new() -> Self {
    // no match happened yet
    Self(AtomicBool::new(false))
  }
}

// TODO: refactor this shitty code.
impl Printer for JSONPrinter {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SupportLang>,
  ) {
    let mut matches = matches.peekable();
    if matches.peek().is_none() {
      return;
    }
    let mut lock = std::io::stdout();
    let matched = self.0.swap(true, Ordering::AcqRel);
    let path = file.name();
    if !matched {
      writeln!(&mut lock, "[").unwrap();
      let nm = matches.next().unwrap();
      let v = RuleMatchJSON::new(nm, path, rule);
      serde_json::to_writer_pretty(&mut lock, &v).unwrap();
    }
    for nm in matches {
      writeln!(&mut lock, ",").unwrap();
      let v = RuleMatchJSON::new(nm, path, rule);
      serde_json::to_writer_pretty(&mut lock, &v).unwrap();
    }
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    let mut matches = matches.peekable();
    if matches.peek().is_none() {
      return Ok(());
    }
    let mut lock = std::io::stdout();
    let matched = self.0.swap(true, Ordering::AcqRel);
    let path = path.to_string_lossy();
    if !matched {
      writeln!(&mut lock, "[").unwrap();
      let nm = matches.next().unwrap();
      let v = MatchJSON::new(nm, &path);
      serde_json::to_writer_pretty(&mut lock, &v).unwrap();
    }
    for nm in matches {
      writeln!(&mut lock, ",").unwrap();
      let v = MatchJSON::new(nm, &path);
      serde_json::to_writer_pretty(&mut lock, &v).unwrap();
    }
    Ok(())
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    let mut diffs = diffs.peekable();
    if diffs.peek().is_none() {
      return Ok(());
    }
    let mut lock = std::io::stdout();
    let matched = self.0.swap(true, Ordering::AcqRel);
    let path = path.to_string_lossy();
    if !matched {
      writeln!(&mut lock, "[").unwrap();
      let diff = diffs.next().unwrap();
      let mut v = MatchJSON::new(diff.node_match, &path);
      v.replacement = Some(diff.replacement);
      serde_json::to_writer_pretty(&mut lock, &v).unwrap();
    }
    for diff in diffs {
      writeln!(&mut lock, ",").unwrap();
      let mut v = MatchJSON::new(diff.node_match, &path);
      v.replacement = Some(diff.replacement);
      serde_json::to_writer_pretty(&mut lock, &v).unwrap();
    }
    Ok(())
  }
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    _rule: &RuleConfig<SupportLang>,
  ) -> Result<()> {
    self.print_diffs(diffs, path)
  }

  fn after_print(&self) {
    let matched = self.0.load(Ordering::Acquire);
    if matched {
      println!();
    } else {
      print!("[");
    }
    println!("]");
  }
}
