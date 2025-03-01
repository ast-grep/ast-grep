use super::{Diff, NodeMatch, Printer};
use crate::lang::SgLang;
use crate::utils::DiffStyles;
use ansi_term::{Color, Style};
use anyhow::Result;
use ast_grep_config::{RuleConfig, Severity};
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream, WriteColor};
use codespan_reporting::term::{self, DisplayStyle};
use similar::{ChangeTag, DiffOp, TextDiff};

use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;

mod match_merger;
mod styles;
mod test;

use match_merger::MatchMerger;
pub use styles::should_use_color;
use styles::{PrintStyles, RuleStyle};

#[derive(Clone, Copy, ValueEnum)]
pub enum ReportStyle {
  /// Output a richly formatted diagnostic, with source code previews.
  Rich,
  /// Output a condensed diagnostic, with a line number, severity, message and notes (if any).
  Medium,
  /// Output a short diagnostic, with a line number, severity, and message.
  Short,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Heading {
  /// Print heading for terminal tty but not for piped output
  Auto,
  /// Always print heading regardless of output type.
  Always,
  /// Never print heading regardless of output type.
  Never,
}

impl Heading {
  fn should_print(&self) -> bool {
    use Heading as H;
    match self {
      H::Always => true,
      H::Never => false,
      H::Auto => atty::is(atty::Stream::Stdout),
    }
  }
}

pub struct ColoredPrinter<W: WriteColor> {
  writer: W,
  config: term::Config,
  styles: PrintStyles,
  heading: Heading,
  context: (u16, u16),
}
impl ColoredPrinter<StandardStream> {
  pub fn stdout<C: Into<ColorChoice>>(color: C) -> Self {
    let color = color.into();
    ColoredPrinter::new(StandardStream::stdout(color)).color(color)
  }
}

impl<W: WriteColor> ColoredPrinter<W> {
  pub fn new(writer: W) -> Self {
    Self {
      writer,
      styles: PrintStyles::from(ColorChoice::Auto),
      config: term::Config::default(),
      heading: Heading::Auto,
      context: (0, 0),
    }
  }

  pub fn color<C: Into<ColorChoice>>(mut self, color: C) -> Self {
    let color = color.into();
    self.styles = PrintStyles::from(color);
    self
  }

  pub fn style(mut self, style: ReportStyle) -> Self {
    let display_style = match style {
      ReportStyle::Rich => DisplayStyle::Rich,
      ReportStyle::Medium => DisplayStyle::Medium,
      ReportStyle::Short => DisplayStyle::Short,
    };
    self.config.display_style = display_style;
    self
  }

  pub fn heading(mut self, heading: Heading) -> Self {
    self.heading = heading;
    self
  }

  pub fn context(mut self, context: (u16, u16)) -> Self {
    self.context = context;
    self.config.start_context_lines = context.0 as usize;
    self.config.end_context_lines = context.1 as usize;
    self
  }

  fn context_span(&self) -> usize {
    (self.context.0 + self.context.1) as usize
  }

  fn diff_context(&self) -> usize {
    if self.context.0 == 0 {
      3
    } else {
      self.context.0 as usize
    }
  }
}

impl<W: WriteColor> Printer for ColoredPrinter<W> {
  fn print_rule(
    &mut self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    let config = &self.config;
    let writer = &mut self.writer;
    let severity = match rule.severity {
      Severity::Error => diagnostic::Severity::Error,
      Severity::Warning => diagnostic::Severity::Warning,
      Severity::Info => diagnostic::Severity::Note,
      Severity::Hint => diagnostic::Severity::Help,
      Severity::Off => unreachable!("turned-off rule should not have match."),
    };
    for m in matches {
      let range = m.range();
      let mut labels = vec![Label::primary((), range)];
      if let Some(secondary_nodes) = m.get_env().get_labels("secondary") {
        labels.extend(secondary_nodes.iter().map(|n| {
          let range = n.range();
          Label::secondary((), range)
        }));
      }
      let diagnostic = Diagnostic::new(severity)
        .with_code(&rule.id)
        .with_message(rule.get_message(&m))
        .with_notes(rule.note.iter().cloned().collect())
        .with_labels(labels);
      term::emit(&mut *writer, config, &file, &diagnostic)?;
    }
    Ok(())
  }

  fn print_matches(&mut self, matches: Vec<NodeMatch>, path: &Path) -> Result<()> {
    if self.heading.should_print() {
      print_matches_with_heading(matches, path, self)
    } else {
      print_matches_with_prefix(matches, path, self)
    }
  }

  fn print_diffs(&mut self, diffs: Vec<Diff>, path: &Path) -> Result<()> {
    let context = self.diff_context();
    let writer = &mut self.writer;
    print_diffs(diffs, path, &self.styles, writer, context)
  }
  fn print_rule_diffs(
    &mut self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<()> {
    let context = self.diff_context();
    let writer = &mut self.writer;
    let mut start = 0;
    print_prelude(path, &self.styles, writer)?;
    for (diff, rule) in diffs {
      let range = &diff.range;
      // skip overlapping diff
      if range.start < start {
        continue;
      }
      start = range.end;
      print_rule_title(rule, &diff.node_match, &self.styles.rule, writer)?;
      let source = diff.get_root_text();
      let new_str = format!(
        "{}{}{}",
        &source[..range.start],
        diff.replacement,
        &source[start..],
      );
      print_diff(source, &new_str, &self.styles.diff, writer, context)?;
      if let Some(note) = &rule.note {
        writeln!(writer, "{}", self.styles.rule.note.paint("Note:"))?;
        writeln!(writer, "{note}")?;
      }
    }
    Ok(())
  }
}

fn print_rule_title<W: WriteColor>(
  rule: &RuleConfig<SgLang>,
  nm: &NodeMatch,
  style: &RuleStyle,
  writer: &mut W,
) -> Result<()> {
  let (level, level_style) = match rule.severity {
    Severity::Error => ("error", style.error),
    Severity::Warning => ("warning", style.warning),
    Severity::Info => ("note", style.info),
    Severity::Hint => ("help", style.hint),
    Severity::Off => unreachable!("turned-off rule should not have match."),
  };
  let header = format!("{level}[{}]:", &rule.id);
  let header = level_style.paint(header);
  let message = style.message.paint(rule.get_message(nm));
  writeln!(writer, "{header} {message}")?;
  Ok(())
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

fn print_prelude(path: &Path, styles: &PrintStyles, writer: &mut impl Write) -> Result<()> {
  let filepath = adjust_dir_separator(path);
  writeln!(writer, "{}", styles.file_path.paint(filepath))?;
  Ok(())
}

fn print_matches_with_heading<W: WriteColor>(
  matches: Vec<NodeMatch>,
  path: &Path,
  printer: &mut ColoredPrinter<W>,
) -> Result<()> {
  let mut matches = matches.into_iter();
  let styles = &printer.styles;
  let context_span = printer.context_span();
  let writer = &mut printer.writer;
  print_prelude(path, styles, writer)?;
  let Some(first_match) = matches.next() else {
    return Ok(());
  };
  let source = first_match.root().get_text();

  let mut merger = MatchMerger::new(&first_match, printer.context);

  let display = merger.display(&first_match);
  let mut ret = display.leading.to_string();
  styles.push_matched_to_ret(&mut ret, &display.matched)?;

  for nm in matches {
    if merger.check_overlapping(&nm) {
      continue;
    }
    let display = merger.display(&nm);
    // merge adjacent matches
    if let Some(last_end_offset) = merger.merge_adjacent(&nm) {
      ret.push_str(&source[last_end_offset..nm.range().start]);
      styles.push_matched_to_ret(&mut ret, &display.matched)?;
      continue;
    }
    ret.push_str(merger.last_trailing);
    let lines = ret.lines().count();
    let mut num = merger.last_start_line;
    let width = (lines + num).checked_ilog10().unwrap_or(0) as usize + 1;
    let line_num = styles.diff.line_num.paint(format!("{num}"));
    write!(writer, "{line_num:>width$}│")?; // initial line num
    print_highlight(ret.lines(), width, &mut num, writer, styles)?;
    writeln!(writer)?; // end match new line
    if context_span > 0 {
      writeln!(writer, "{:╴>width$}┤", "")?; // make separation
    }
    merger.conclude_match(&nm);
    ret = display.leading.to_string();
    styles.push_matched_to_ret(&mut ret, &display.matched)?;
  }
  ret.push_str(merger.last_trailing);
  let lines = ret.lines().count();
  let mut num = merger.last_start_line;
  let width = (lines + num).checked_ilog10().unwrap_or(0) as usize + 1;
  let line_num = styles.diff.line_num.paint(format!("{num}"));
  write!(writer, "{line_num:>width$}│")?; // initial line num
  print_highlight(ret.lines(), width, &mut num, writer, styles)?;
  writeln!(writer)?; // end match new line
  writeln!(writer)?; // end
  Ok(())
}

fn print_matches_with_prefix<W: WriteColor>(
  matches: Vec<NodeMatch>,
  path: &Path,
  printer: &mut ColoredPrinter<W>,
) -> Result<()> {
  let mut matches = matches.into_iter();
  let styles = &printer.styles;
  let context_span = printer.context_span();
  let writer = &mut printer.writer;
  let path = path.display();
  let Some(first_match) = matches.next() else {
    return Ok(());
  };
  let source = first_match.root().get_text();

  let mut merger = MatchMerger::new(&first_match, printer.context);
  let display = merger.display(&first_match);
  let mut ret = display.leading.to_string();
  styles.push_matched_to_ret(&mut ret, &display.matched)?;
  for nm in matches {
    if merger.check_overlapping(&nm) {
      continue;
    }
    let display = merger.display(&nm);
    // merge adjacent matches
    if let Some(last_end_offset) = merger.merge_adjacent(&nm) {
      ret.push_str(&source[last_end_offset..nm.range().start]);
      styles.push_matched_to_ret(&mut ret, &display.matched)?;
      continue;
    }
    ret.push_str(merger.last_trailing);
    for (n, line) in ret.lines().enumerate() {
      let num = merger.last_start_line + n;
      writeln!(writer, "{path}:{num}:{line}")?;
    }
    if context_span > 0 {
      writeln!(writer, "--")?; // make separation
    }
    merger.conclude_match(&nm);
    ret = display.leading.to_string();
    styles.push_matched_to_ret(&mut ret, &display.matched)?;
  }
  ret.push_str(merger.last_trailing);
  for (n, line) in ret.lines().enumerate() {
    let num = merger.last_start_line + n;
    writeln!(writer, "{path}:{num}:{line}")?;
  }
  Ok(())
}

fn print_diffs<W: WriteColor>(
  diffs: Vec<Diff>,
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
  context: usize,
) -> Result<()> {
  let mut diffs = diffs.into_iter();
  print_prelude(path, styles, writer)?;
  let Some(first_diff) = diffs.next() else {
    return Ok(());
  };
  let source = first_diff.get_root_text();
  let range = first_diff.range;
  let mut start = range.end;
  let mut new_str = format!("{}{}", &source[..range.start], first_diff.replacement);
  for diff in diffs {
    let range = diff.range;
    // skip overlapping diff
    if range.start < start {
      continue;
    }
    new_str.push_str(&source[start..range.start]);
    new_str.push_str(&diff.replacement);
    start = range.end;
  }
  new_str.push_str(&source[start..]);
  print_diff(source, &new_str, &styles.diff, writer, context)?;
  Ok(())
}

fn print_highlight<'a, W: Write>(
  mut lines: impl Iterator<Item = &'a str>,
  width: usize,
  num: &mut usize,
  writer: &mut W,
  styles: &PrintStyles,
) -> Result<()> {
  if let Some(line) = lines.next() {
    write!(writer, "{line}")?;
  }
  for line in lines {
    writeln!(writer)?;
    *num += 1;
    let line_num = styles.diff.line_num.paint(format!("{num}"));
    write!(writer, "{line_num:>width$}│{line}")?;
  }
  Ok(())
}

fn index_display(index: Option<usize>, style: Style, width: usize) -> impl Display {
  let index_str = match index {
    None => format!("{:width$}", ""),
    Some(idx) => format!("{:<width$}", idx + 1), // 0-based index -> 1-based line num
  };
  style.paint(index_str)
}

// TODO: currently diff print context is three lines before/after the match.
// This is suboptimal. We should use function/class as the enclosing scope to print relevant lines. See #155
fn compute_header(group: &[DiffOp]) -> String {
  let old_start = group[0].old_range().start;
  let new_start = group[0].new_range().start;
  let (old_len, new_len) = group.iter().fold((0, 0), |(o, n), op| {
    (o + op.old_range().len(), n + op.new_range().len())
  });
  format!(
    "@@ -{},{} +{},{} @@",
    old_start, old_len, new_start, new_len
  )
}

pub fn print_diff(
  old: &str,
  new: &str,
  styles: &DiffStyles,
  writer: &mut impl Write,
  context: usize,
) -> Result<()> {
  let diff = TextDiff::from_lines(old, new);
  for group in diff.grouped_ops(context) {
    let op = group.last().unwrap();
    let old_width = op.old_range().end.checked_ilog10().unwrap_or(0) as usize + 1;
    let new_width = op.new_range().end.checked_ilog10().unwrap_or(0) as usize + 1;
    let header = compute_header(&group);
    writeln!(writer, "{}", Color::Blue.paint(header))?;
    for op in group {
      for change in diff.iter_inline_changes(&op) {
        let (sign, s, em, line_num) = match change.tag() {
          ChangeTag::Delete => ("-", styles.delete, styles.delete_emphasis, styles.delete),
          ChangeTag::Insert => ("+", styles.insert, styles.insert_emphasis, styles.insert),
          ChangeTag::Equal => (" ", Style::new(), Style::new(), styles.line_num),
        };
        write!(
          writer,
          "{} {}│{}",
          index_display(change.old_index(), line_num, old_width),
          index_display(change.new_index(), line_num, new_width),
          s.paint(sign),
        )?;
        for (emphasized, value) in change.iter_strings_lossy() {
          if emphasized {
            write!(writer, "{}", em.paint(value))?;
          } else {
            write!(writer, "{}", s.paint(value))?;
          }
        }
        if change.missing_newline() {
          writeln!(writer)?;
        }
      }
    }
  }
  Ok(())
}
