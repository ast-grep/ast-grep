use super::{Diff, NodeMatch, Printer};
use crate::lang::SgLang;
use crate::utils::DiffStyles;
use anyhow::Result;
use ast_grep_config::{RuleConfig, Severity};
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream, WriteColor};
use codespan_reporting::term::{self, DisplayStyle};

use std::borrow::Cow;
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
    self.styles.print_prelude(path, writer)?;
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
      self
        .styles
        .diff
        .print_diff(source, &new_str, writer, context)?;
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

fn print_matches_with_heading<W: WriteColor>(
  matches: Vec<NodeMatch>,
  path: &Path,
  printer: &mut ColoredPrinter<W>,
) -> Result<()> {
  let mut matches = matches.into_iter();
  let styles = &printer.styles;
  let context_span = printer.context_span();
  let writer = &mut printer.writer;
  styles.print_prelude(path, writer)?;
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
    let num = merger.last_start_line;
    let width = print_highlight(&ret, num, writer, styles)?;
    if context_span > 0 {
      writeln!(writer, "{:╴>width$}┤", "")?; // make separation
    }
    merger.conclude_match(&nm);
    ret = display.leading.to_string();
    styles.push_matched_to_ret(&mut ret, &display.matched)?;
  }
  ret.push_str(merger.last_trailing);
  let num = merger.last_start_line;
  print_highlight(&ret, num, writer, styles)?;
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
  styles.print_prelude(path, writer)?;
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
  styles.diff.print_diff(source, &new_str, writer, context)?;
  Ok(())
}

fn print_highlight<W: Write>(
  ret: &str,
  start_line: usize,
  writer: &mut W,
  styles: &PrintStyles,
) -> Result<usize> {
  let added = ret.lines().count();
  // compute width for line number. log10(num) = the digit count of num - 1
  let width = (added + start_line).checked_ilog10().unwrap_or(0) as usize + 1;
  for (offset, line) in ret.lines().enumerate() {
    // note the width modifier must be applied before coloring the line_num
    let line_num = format!("{:<width$}", start_line + offset);
    // otherwise the color ascii code will be counted in the width
    let ln_text = styles.diff.line_num.paint(line_num);
    writeln!(writer, "{ln_text}│{line}")?;
  }
  Ok(width)
}
