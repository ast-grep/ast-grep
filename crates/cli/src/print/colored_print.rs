use super::{Diff, Printer};
use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::NodeMatch;
use ast_grep_language::SupportLang;

use ansi_term::{Color, Style};
use anyhow::Result;
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream, WriteColor};
use codespan_reporting::term::{self, DisplayStyle};
pub use codespan_reporting::{files::SimpleFile, term::ColorArg};
use similar::{ChangeTag, TextDiff};

use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SupportLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ReportStyle {
  Rich,
  Medium,
  Short,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Heading {
  Always,
  Never,
  Auto,
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
  writer: Mutex<W>,
  config: term::Config,
  styles: PrintStyles,
  heading: Heading,
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
      writer: Mutex::new(writer),
      styles: PrintStyles::from(ColorChoice::Auto),
      config: term::Config::default(),
      heading: Heading::Auto,
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
}

impl<W: WriteColor> Printer for ColoredPrinter<W> {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SupportLang>,
  ) {
    let config = &self.config;
    let mut writer = self.writer.lock().expect("should not fail");
    let serverity = match rule.severity {
      Severity::Error => diagnostic::Severity::Error,
      Severity::Warning => diagnostic::Severity::Warning,
      Severity::Info => diagnostic::Severity::Note,
      Severity::Hint => diagnostic::Severity::Help,
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
      let diagnostic = Diagnostic::new(serverity)
        .with_code(&rule.id)
        .with_message(rule.get_message(&m))
        .with_notes(rule.note.iter().cloned().collect())
        .with_labels(labels);
      term::emit(&mut *writer, config, &file, &diagnostic).unwrap();
    }
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    let writer = &mut *self.writer.lock().expect("should success");
    if self.heading.should_print() {
      print_matches_with_heading(matches, path, &self.styles, writer)
    } else {
      print_matches_with_prefix(matches, path, &self.styles, writer)
    }
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    let writer = &mut *self.writer.lock().expect("should success");
    print_diffs(diffs, path, &self.styles, writer)
  }
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    _rule: &RuleConfig<SupportLang>,
  ) -> Result<()> {
    self.print_diffs(diffs, path)
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

fn print_prelude(path: &Path, styles: &PrintStyles, writer: &mut impl Write) -> Result<()> {
  let filepath = adjust_dir_separator(path);
  writeln!(writer, "{}", styles.file_path.paint(filepath))?;
  Ok(())
}

fn print_matches_with_heading<'a, W: Write>(
  matches: Matches!('a),
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
) -> Result<()> {
  print_prelude(path, styles, writer)?;
  for e in matches {
    let display = e.display_context(0);
    let leading = display.leading;
    let trailing = display.trailing;
    let matched = display.matched;
    let highlighted = format!("{leading}{matched}{trailing}");
    let lines = highlighted.lines().count();
    let mut num = display.start_line;
    let width = (lines + display.start_line).to_string().chars().count();
    write!(writer, "{num:>width$}│")?; // initial line num
    print_highlight(leading.lines(), Style::new(), width, &mut num, writer)?;
    print_highlight(matched.lines(), styles.matched, width, &mut num, writer)?;
    print_highlight(trailing.lines(), Style::new(), width, &mut num, writer)?;
    writeln!(writer)?; // end match new line
  }
  Ok(())
}

fn print_matches_with_prefix<'a, W: WriteColor>(
  matches: Matches!('a),
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
) -> Result<()> {
  let path = path.display();
  for e in matches {
    let display = e.display_context(0);
    let leading = display.leading;
    let trailing = display.trailing;
    let matched = styles.matched.paint(display.matched);
    let highlighted = format!("{leading}{matched}{trailing}");
    for (n, line) in highlighted.lines().enumerate() {
      let num = display.start_line + n;
      writeln!(writer, "{path}:{num}:{line}")?;
    }
  }
  Ok(())
}

fn print_diffs<'a, W: WriteColor>(
  diffs: Diffs!('a),
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
) -> Result<()> {
  print_prelude(path, styles, writer)?;
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
    print_diff(&old_str, &new_str, base_line, styles, writer)?;
  }
  Ok(())
}

fn print_highlight<'a, W: Write>(
  mut lines: impl Iterator<Item = &'a str>,
  style: Style,
  width: usize,
  num: &mut usize,
  writer: &mut W,
) -> Result<()> {
  if let Some(line) = lines.next() {
    let line = style.paint(line);
    write!(writer, "{line}")?;
  }
  for line in lines {
    writeln!(writer)?;
    *num += 1;
    let line = style.paint(line);
    write!(writer, "{num:>width$}│{line}")?;
  }
  Ok(())
}

fn index_display(index: Option<usize>, style: Style, width: usize) -> impl Display {
  let index_str = match index {
    None => format!("{:width$}", ""),
    Some(idx) => format!("{:<width$}", idx),
  };
  style.paint(index_str)
}

pub fn print_diff(
  old: &str,
  new: &str,
  base_line: usize,
  styles: &PrintStyles,
  writer: &mut impl Write,
) -> Result<()> {
  let diff = TextDiff::from_lines(old, new);
  let width = base_line.to_string().chars().count();
  for (idx, group) in diff.grouped_ops(5).iter().enumerate() {
    if idx > 0 {
      writeln!(writer, "{:-^1$}", "-", 80)?;
    }
    for op in group {
      for change in diff.iter_inline_changes(op) {
        let (sign, s, em) = match change.tag() {
          ChangeTag::Delete => ("-", styles.delete, styles.delete_emphasis),
          ChangeTag::Insert => ("+", styles.insert, styles.insert_emphasis),
          ChangeTag::Equal => (" ", Style::new(), Style::new()),
        };
        write!(
          writer,
          "{}{}|{}",
          index_display(change.old_index().map(|i| i + base_line), s, width + 1),
          index_display(change.new_index().map(|i| i + base_line), s, width),
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

// TODO: use termcolor instead
#[derive(Default)]
pub struct PrintStyles {
  file_path: Style,
  matched: Style,
  insert: Style,
  insert_emphasis: Style,
  delete: Style,
  delete_emphasis: Style,
}

impl PrintStyles {
  fn colored() -> Self {
    static THISTLE1: Color = Color::Fixed(225);
    static SEA_GREEN: Color = Color::Fixed(158);
    static RED: Color = Color::Fixed(161);
    static GREEN: Color = Color::Fixed(35);
    Self {
      file_path: Color::Cyan.italic(),
      matched: Color::Red.bold(),
      insert: Style::new().fg(GREEN),
      insert_emphasis: Style::new().fg(GREEN).on(SEA_GREEN).bold(),
      delete: Style::new().fg(RED),
      delete_emphasis: Style::new().fg(RED).on(THISTLE1).bold(),
    }
  }
  fn no_color() -> Self {
    Self::default()
  }
}
impl From<ColorChoice> for PrintStyles {
  fn from(color: ColorChoice) -> Self {
    if choose_color::should_use_color(&color) {
      Self::colored()
    } else {
      Self::no_color()
    }
  }
}

// copied from termcolor
mod choose_color {
  use super::ColorChoice;
  use std::env;
  /// Returns true if we should attempt to write colored output.
  pub fn should_use_color(color: &ColorChoice) -> bool {
    match *color {
      ColorChoice::Always => env_allow_ansi(),
      ColorChoice::AlwaysAnsi => true,
      ColorChoice::Never => false,
      ColorChoice::Auto => env_allows_color(),
    }
  }

  #[cfg(not(windows))]
  fn env_allows_color() -> bool {
    match env::var_os("TERM") {
      // If TERM isn't set, then we are in a weird environment that
      // probably doesn't support colors.
      None => return false,
      Some(k) => {
        if k == "dumb" {
          return false;
        }
      }
    }
    // If TERM != dumb, then the only way we don't allow colors at this
    // point is if NO_COLOR is set.
    if env::var_os("NO_COLOR").is_some() {
      return false;
    }
    env_allow_ansi()
  }

  #[cfg(windows)]
  fn env_allows_color() -> bool {
    // On Windows, if TERM isn't set, then we shouldn't automatically
    // assume that colors aren't allowed. This is unlike Unix environments
    // where TERM is more rigorously set.
    if let Some(k) = env::var_os("TERM") {
      if k == "dumb" {
        return false;
      }
    }
    // If TERM != dumb, then the only way we don't allow colors at this
    // point is if NO_COLOR is set.
    if env::var_os("NO_COLOR").is_some() {
      return false;
    }
    env_allow_ansi()
  }

  #[cfg(not(windows))]
  fn env_allow_ansi() -> bool {
    true
  }

  /// Returns true if this choice should forcefully use ANSI color codes.
  ///
  /// It's possible that ANSI is still the correct choice even if this
  /// returns false.
  #[cfg(windows)]
  fn env_allow_ansi() -> bool {
    match *self {
      ColorChoice::Always => false,
      ColorChoice::AlwaysAnsi => true,
      ColorChoice::Never => false,
      ColorChoice::Auto => {
        match env::var("TERM") {
          Err(_) => false,
          // cygwin doesn't seem to support ANSI escape sequences
          // and instead has its own variety. However, the Windows
          // console API may be available.
          Ok(k) => k != "dumb" && k != "cygwin",
        }
      }
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::language::Language;
  use codespan_reporting::term::termcolor::Buffer;

  fn make_test_printer() -> ColoredPrinter<Buffer> {
    ColoredPrinter::new(Buffer::no_color()).color(ColorChoice::Never)
  }
  fn get_text(printer: &ColoredPrinter<Buffer>) -> String {
    let buffer = printer.writer.lock().expect("should work");
    let bytes = buffer.as_slice();
    std::str::from_utf8(bytes)
      .expect("buffer should be valid utf8")
      .to_owned()
  }

  #[test]
  fn test_emtpy_printer() {
    let printer = make_test_printer();
    assert_eq!(get_text(&printer), "");
  }

  #[test]
  fn test_printe_matches() {
    let printer = make_test_printer();
    let grep = SupportLang::Tsx.ast_grep("let a = 123");
    let matches = grep.root().find_all("a");
    printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
    let expected = "test.tsx\n1│let a = 123\n";
    assert_eq!(get_text(&printer), expected);
  }

  #[test]
  #[ignore]
  fn test_printe_rules() {
    todo!()
  }

  #[test]
  #[ignore]
  fn test_printe_diffs() {
    todo!()
  }
}
