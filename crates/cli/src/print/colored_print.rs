use super::{Diff, Printer};
use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};

use ansi_term::{Color, Style};
use anyhow::Result;
use clap::ValueEnum;
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream, WriteColor};
use codespan_reporting::term::{self, DisplayStyle};
pub use codespan_reporting::{files::SimpleFile, term::ColorArg};
use similar::{ChangeTag, DiffOp, TextDiff};

use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use ast_grep_core::{NodeMatch as SgNodeMatch, StrDoc};
type NodeMatch<'a, L> = SgNodeMatch<'a, StrDoc<L>>;

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SgLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

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
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    let config = &self.config;
    let mut writer = self.writer.lock().expect("should not fail");
    let serverity = match rule.severity {
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
      let diagnostic = Diagnostic::new(serverity)
        .with_code(&rule.id)
        .with_message(rule.get_message(&m))
        .with_notes(rule.note.iter().cloned().collect())
        .with_labels(labels);
      term::emit(&mut *writer, config, &file, &diagnostic)?;
    }
    Ok(())
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
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    let writer = &mut *self.writer.lock().expect("should success");
    print_rule_title(rule, &self.styles.rule, writer)?;
    print_diffs(diffs, path, &self.styles, writer)?;
    if let Some(note) = &rule.note {
      writeln!(writer, "{}", self.styles.rule.note.paint("Note:"))?;
      writeln!(writer, "{note}")?;
    }
    Ok(())
  }
}

fn print_rule_title<W: WriteColor>(
  rule: &RuleConfig<SgLang>,
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
  let message = style.message.paint(&rule.message);
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

// merging overlapping/adjacent matches
// adjacent matches: matches that starts or ends on the same line
struct MatchMerger<'a> {
  last_start_line: usize,
  last_end_line: usize,
  last_trailing: &'a str,
  last_end_offset: usize,
}

impl<'a> MatchMerger<'a> {
  fn new(nm: &NodeMatch<'a, SgLang>) -> Self {
    let display = nm.display_context(0);
    let last_start_line = display.start_line;
    let last_end_line = nm.end_pos().0;
    let last_trailing = display.trailing;
    let last_end_offset = nm.range().end;
    Self {
      last_start_line,
      last_end_line,
      last_end_offset,
      last_trailing,
    }
  }

  // merge non-overlapping matches but start/end on the same line
  fn merge_adjacent(&mut self, nm: &NodeMatch<'a, SgLang>) -> Option<usize> {
    let start_line = nm.start_pos().0;
    let display = nm.display_context(0);
    if start_line == self.last_end_line {
      let last_end_offset = self.last_end_offset;
      self.last_end_offset = nm.range().end;
      self.last_trailing = display.trailing;
      Some(last_end_offset)
    } else {
      None
    }
  }

  fn conclude_match(&mut self, nm: &NodeMatch<'a, SgLang>) {
    let display = nm.display_context(0);
    self.last_start_line = display.start_line;
    self.last_end_line = nm.end_pos().0;
    self.last_trailing = display.trailing;
    self.last_end_offset = nm.range().end;
  }

  #[inline]
  fn check_overlapping(&self, nm: &NodeMatch<'a, SgLang>) -> bool {
    let range = nm.range();

    // merge overlapping matches.
    // N.B. range.start == last_end_offset does not mean overlapping
    if range.start < self.last_end_offset {
      // guaranteed by pre-order
      debug_assert!(range.end <= self.last_end_offset);
      true
    } else {
      false
    }
  }
}

fn print_matches_with_heading<'a, W: Write>(
  mut matches: Matches!('a),
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
) -> Result<()> {
  print_prelude(path, styles, writer)?;
  let Some(first_match) = matches.next() else {
    return Ok(())
  };
  let source = first_match.root().get_text();
  let display = first_match.display_context(0);

  let mut merger = MatchMerger::new(&first_match);
  let mut ret = display.leading.to_string();
  styles.push_matched_to_ret(&mut ret, &display.matched)?;

  for nm in matches {
    if merger.check_overlapping(&nm) {
      continue;
    }
    let display = nm.display_context(0);
    // merge adjacent matches
    if let Some(last_end_offset) = merger.merge_adjacent(&nm) {
      ret.push_str(&source[last_end_offset..nm.range().start]);
      styles.push_matched_to_ret(&mut ret, &display.matched)?;
      continue;
    }
    ret.push_str(merger.last_trailing);
    let lines = ret.lines().count();
    let mut num = merger.last_start_line;
    let width = (lines + num).to_string().chars().count();
    write!(writer, "{num:>width$}│")?; // initial line num
    print_highlight(ret.lines(), width, &mut num, writer)?;
    writeln!(writer)?; // end match new line
                       //
    merger.conclude_match(&nm);
    ret = display.leading.to_string();
    styles.push_matched_to_ret(&mut ret, &display.matched)?;
  }
  ret.push_str(merger.last_trailing);
  let lines = ret.lines().count();
  let mut num = merger.last_start_line;
  let width = (lines + num).to_string().chars().count();
  write!(writer, "{num:>width$}│")?; // initial line num
  print_highlight(ret.lines(), width, &mut num, writer)?;
  writeln!(writer)?; // end match new line
  writeln!(writer)?; // end
  Ok(())
}

fn print_matches_with_prefix<'a, W: WriteColor>(
  mut matches: Matches!('a),
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
) -> Result<()> {
  let path = path.display();
  let Some(first_match) = matches.next() else {
    return Ok(())
  };
  let source = first_match.root().get_text();
  let display = first_match.display_context(0);

  let mut merger = MatchMerger::new(&first_match);
  let mut ret = display.leading.to_string();
  styles.push_matched_to_ret(&mut ret, &display.matched)?;
  for nm in matches {
    if merger.check_overlapping(&nm) {
      continue;
    }
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
    merger.conclude_match(&nm);
    let display = nm.display_context(0);
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

fn print_diffs<'a, W: WriteColor>(
  mut diffs: Diffs!('a),
  path: &Path,
  styles: &PrintStyles,
  writer: &mut W,
) -> Result<()> {
  print_prelude(path, styles, writer)?;
  let Some(first_diff) = diffs.next() else {
    return Ok(());
  };
  let range = first_diff.node_match.range();
  let source = first_diff.node_match.root().get_text();
  let mut start = range.end;
  let mut new_str = format!("{}{}", &source[..range.start], first_diff.replacement);
  for diff in diffs {
    let range = diff.node_match.range();
    // skip overlapping diff
    if range.start < start {
      continue;
    }
    new_str.push_str(&source[start..range.start]);
    new_str.push_str(&diff.replacement);
    start = range.end;
  }
  new_str.push_str(&source[start..]);
  print_diff(source, &new_str, styles, writer)?;
  Ok(())
}

fn print_highlight<'a, W: Write>(
  mut lines: impl Iterator<Item = &'a str>,
  width: usize,
  num: &mut usize,
  writer: &mut W,
) -> Result<()> {
  if let Some(line) = lines.next() {
    write!(writer, "{line}")?;
  }
  for line in lines {
    writeln!(writer)?;
    *num += 1;
    write!(writer, "{num:>width$}│{line}")?;
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
fn compupte_header(group: &[DiffOp]) -> String {
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
  styles: &PrintStyles,
  writer: &mut impl Write,
) -> Result<()> {
  let diff = TextDiff::from_lines(old, new);
  for group in diff.grouped_ops(3) {
    let op = group.last().unwrap();
    let old_width = op.old_range().end.to_string().chars().count();
    let new_width = op.new_range().end.to_string().chars().count();
    let header = compupte_header(&group);
    writeln!(writer, "{}", Color::Blue.paint(header))?;
    for op in group {
      for change in diff.iter_inline_changes(&op) {
        let (sign, s, em) = match change.tag() {
          ChangeTag::Delete => ("-", styles.delete, styles.delete_emphasis),
          ChangeTag::Insert => ("+", styles.insert, styles.insert_emphasis),
          ChangeTag::Equal => (" ", Style::new(), Style::new()),
        };
        write!(
          writer,
          "{} {}│{}",
          index_display(change.old_index(), s, old_width),
          index_display(change.new_index(), s, new_width),
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

// warn[rule-id]: rule message here.
// |------------|------------------|
//    header            message
#[derive(Default)]
struct RuleStyle {
  // header style
  error: Style,
  warning: Style,
  info: Style,
  hint: Style,
  // message style
  message: Style,
  note: Style,
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
  rule: RuleStyle,
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
      rule: RuleStyle {
        error: Color::Red.bold(),
        warning: Color::Yellow.bold(),
        info: Style::new().bold(),
        hint: Style::new().dimmed().bold(),
        note: Style::new().italic(),
        message: Style::new().bold(),
      },
    }
  }
  fn no_color() -> Self {
    Self::default()
  }

  fn push_matched_to_ret(&self, ret: &mut String, matched: &str) -> Result<()> {
    use std::fmt::Write;
    // TODO: use intersperse
    let mut lines = matched.lines();
    if let Some(line) = lines.next() {
      write!(ret, "{}", self.matched.paint(line))?;
    } else {
      return Ok(());
    }
    for line in lines {
      ret.push('\n');
      write!(ret, "{}", self.matched.paint(line))?;
    }
    Ok(())
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
      ColorChoice::Always => env_allow_ansi(color),
      ColorChoice::AlwaysAnsi => true,
      ColorChoice::Never => false,
      // NOTE tty check is added
      ColorChoice::Auto => atty::is(atty::Stream::Stdout) && env_allows_color(color),
    }
  }

  #[cfg(not(windows))]
  fn env_allows_color(color: &ColorChoice) -> bool {
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
    env_allow_ansi(color)
  }

  #[cfg(windows)]
  fn env_allows_color(color: &ColorChoice) -> bool {
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
    env_allow_ansi(color)
  }

  #[cfg(not(windows))]
  fn env_allow_ansi(_color: &ColorChoice) -> bool {
    true
  }

  /// Returns true if this choice should forcefully use ANSI color codes.
  ///
  /// It's possible that ANSI is still the correct choice even if this
  /// returns false.
  #[cfg(windows)]
  fn env_allow_ansi(color: &ColorChoice) -> bool {
    match *color {
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
  use ast_grep_config::{from_yaml_string, GlobalRules};
  use ast_grep_core::replacer::Fixer;
  use ast_grep_language::{Language, SupportLang};
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

  // source, pattern, debug note
  type Case<'a> = (&'a str, &'a str, &'a str);

  const MATCHES_CASES: &[Case] = &[
    ("let a = 123", "a", "Simple match"),
    ("Some(1), Some(2), Some(3)", "Some", "Same line match"),
    (
      "Some(1), Some(2)\nSome(3), Some(4)",
      "Some",
      "Multiple line match",
    ),
    (
      "import a from 'b';import a from 'b';",
      "import a from 'b';",
      "immediate following but not overlapping",
    ),
    ("Some(Some(123))", "Some($A)", "overlapping"),
  ];
  #[test]
  fn test_print_matches() {
    for &(source, pattern, note) in MATCHES_CASES {
      // heading is required for CI
      let printer = make_test_printer().heading(Heading::Always);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
      let matches = grep.root().find_all(pattern);
      printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
      let expected: String = source
        .lines()
        .enumerate()
        .map(|(i, l)| format!("{}│{l}\n", i + 1))
        .collect();
      // append heading to expected
      let output = format!("test.tsx\n{expected}\n");
      assert_eq!(get_text(&printer), output, "{note}");
    }
  }

  #[test]
  fn test_print_matches_without_heading() {
    for &(source, pattern, note) in MATCHES_CASES {
      let printer = make_test_printer().heading(Heading::Never);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
      let matches = grep.root().find_all(pattern);
      printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
      // append heading to expected
      let output: String = source
        .lines()
        .enumerate()
        .map(|(i, e)| format!("test.tsx:{}:{e}\n", i + 1))
        .collect();
      assert_eq!(get_text(&printer), output, "{note}");
    }
  }

  #[test]
  fn test_print_rules() {
    let globals = GlobalRules::default();
    for &(source, pattern, note) in MATCHES_CASES {
      let printer = make_test_printer()
        .heading(Heading::Never)
        .style(ReportStyle::Short);
      let grep = SgLang::from(SupportLang::TypeScript).ast_grep(source);
      let matches = grep.root().find_all(pattern);
      let source = source.to_string();
      let file = SimpleFile::new(Cow::Borrowed("test.tsx"), &source);
      let rule = from_yaml_string(
        &format!(
          r"
id: test-id
message: test rule
severity: info
language: TypeScript
rule:
  pattern: {pattern}"
        ),
        &globals,
      )
      .expect("should parse")
      .pop()
      .unwrap();
      printer.print_rule(matches, file, &rule).expect("test only");
      let text = get_text(&printer);
      assert!(text.contains("test.tsx"), "{note}");
      assert!(text.contains("note[test-id]"), "{note}");
      assert!(text.contains("test rule"), "{note}");
    }
  }

  // source, pattern, rewrite, debug note
  type DiffCase<'a> = (&'a str, &'a str, &'a str, &'a str);

  const DIFF_CASES: &[DiffCase] = &[
    ("let a = 123", "a", "b", "Simple match"),
    (
      "Some(1), Some(2), Some(3)",
      "Some",
      "Any",
      "Same line match",
    ),
    (
      "Some(1), Some(2)\nSome(3), Some(4)",
      "Some",
      "Any",
      "Multiple line match",
    ),
    (
      "import a from 'b';import a from 'b';",
      "import a from 'b';",
      "",
      "immediate following but not overlapping",
    ),
    (
      "\n\ntest",
      "test",
      "rest",
      // https://github.com/ast-grep/ast-grep/issues/517
      "leading empty space",
    ),
  ];

  #[test]
  fn test_print_diffs() {
    for &(source, pattern, rewrite, note) in DIFF_CASES {
      // heading is required for CI
      let printer = make_test_printer().heading(Heading::Always);
      let lang = SgLang::from(SupportLang::Tsx);
      let fixer = Fixer::try_new(rewrite, &lang).expect("should work");
      let grep = lang.ast_grep(source);
      let matches = grep.root().find_all(pattern);
      let diffs = matches.map(|n| Diff::generate(n, &pattern, &fixer));
      printer.print_diffs(diffs, "test.tsx".as_ref()).unwrap();
      assert!(get_text(&printer).contains(rewrite), "{note}");
    }
  }
}
