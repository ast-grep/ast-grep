use super::{Diff, NodeMatch, PrintProcessor, Printer};
use crate::lang::SgLang;
use crate::utils::DiffStyles;
use anyhow::Result;
use ast_grep_config::{RuleConfig, Severity};
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{Buffer, ColorChoice, StandardStream, WriteColor};
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
}

impl<W: WriteColor> Printer for ColoredPrinter<W> {
  type Processed = Buffer;
  type Processor = ColoredProcessor;

  fn get_processor(&self) -> Self::Processor {
    ColoredProcessor {
      color: self.writer.supports_color(),
      config: self.config.clone(),
      styles: self.styles.clone(),
      heading: self.heading,
      context: self.context,
    }
  }

  fn process(&mut self, buffer: Buffer) -> Result<()> {
    self.writer.write_all(buffer.as_slice())?;
    Ok(())
  }
}

impl ColoredPrinter<StandardStream> {
  pub fn stdout<C: Into<ColorChoice>>(color: C) -> Self {
    let color = color.into();
    ColoredPrinter::new(StandardStream::stdout(color)).color(color)
  }
}

fn create_buffer(color: bool) -> Buffer {
  if color {
    Buffer::ansi()
  } else {
    Buffer::no_color()
  }
}

pub struct ColoredProcessor {
  color: bool,
  config: term::Config,
  styles: PrintStyles,
  heading: Heading,
  context: (u16, u16),
}

impl ColoredProcessor {
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

impl PrintProcessor<Buffer> for ColoredProcessor {
  fn print_rule(
    &self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<Buffer> {
    let config = &self.config;
    let mut buffer = create_buffer(self.color);
    let writer = &mut buffer;
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
    Ok(buffer)
  }

  fn print_matches(&self, matches: Vec<NodeMatch>, path: &Path) -> Result<Buffer> {
    if self.heading.should_print() {
      print_matches_with_heading(matches, path, self)
    } else {
      print_matches_with_prefix(matches, path, self)
    }
  }

  fn print_diffs(&self, diffs: Vec<Diff>, path: &Path) -> Result<Buffer> {
    let context = self.diff_context();
    let mut buffer = create_buffer(self.color);
    let writer = &mut buffer;
    print_diffs(diffs, path, &self.styles, writer, context)?;
    Ok(buffer)
  }
  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<Buffer> {
    let context = self.diff_context();
    let mut buffer = create_buffer(self.color);
    let writer = &mut buffer;
    let mut start = 0;
    let display_style = &self.config.display_style;
    for (diff, rule) in diffs {
      let range = &diff.range;
      // skip overlapping diff
      if range.start < start {
        continue;
      }
      start = range.end;
      if matches!(display_style, DisplayStyle::Rich) {
        self.styles.print_prelude(path, writer)?;
      } else {
        let pos = diff.node_match.start_pos();
        write!(
          writer,
          "{}:{}:{}: ",
          path.display(),
          pos.line() + 1,
          pos.column(&diff.node_match) + 1
        )?;
      }
      print_rule_title(rule, &diff.node_match, &self.styles.rule, writer)?;
      if matches!(display_style, DisplayStyle::Rich) {
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
      }
      if matches!(display_style, DisplayStyle::Medium | DisplayStyle::Rich) {
        if let Some(note) = &rule.note {
          writeln!(writer, "{}", self.styles.rule.note.paint("Note:"))?;
          writeln!(writer, "{note}")?;
        }
      }
    }
    Ok(buffer)
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

fn print_matches_with_heading(
  matches: Vec<NodeMatch>,
  path: &Path,
  printer: &ColoredProcessor,
) -> Result<Buffer> {
  let mut matches = matches.into_iter();
  let styles = &printer.styles;
  let context_span = printer.context_span();
  let mut buffer = create_buffer(printer.color);
  let writer = &mut buffer;
  styles.print_prelude(path, writer)?;
  let Some(first_match) = matches.next() else {
    return Ok(buffer);
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
    let width = styles.print_highlight(&ret, num, writer)?;
    if context_span > 0 {
      writeln!(writer, "{:╴>width$}┤", "")?; // make separation
    }
    merger.conclude_match(&nm);
    ret = display.leading.to_string();
    styles.push_matched_to_ret(&mut ret, &display.matched)?;
  }
  ret.push_str(merger.last_trailing);
  let num = merger.last_start_line;
  styles.print_highlight(&ret, num, writer)?;
  writeln!(writer)?; // end
  Ok(buffer)
}

fn print_matches_with_prefix(
  matches: Vec<NodeMatch>,
  path: &Path,
  printer: &ColoredProcessor,
) -> Result<Buffer> {
  let mut matches = matches.into_iter();
  let styles = &printer.styles;
  let context_span = printer.context_span();
  let mut buffer = create_buffer(printer.color);
  let writer = &mut buffer;
  let path = path.display();
  let Some(first_match) = matches.next() else {
    return Ok(buffer);
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
  Ok(buffer)
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
