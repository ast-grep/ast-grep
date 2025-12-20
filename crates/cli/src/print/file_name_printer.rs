use super::{Diff, NodeMatch, PrintProcessor, Printer};
use crate::lang::SgLang;
use anyhow::Result;
use ast_grep_config::RuleConfig;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{Buffer, ColorChoice, StandardStream, WriteColor};

use std::borrow::Cow;
use std::path::Path;

use super::PrintStyles;

pub struct FileNamePrinter<W: WriteColor> {
  writer: W,
  styles: PrintStyles,
}

impl<W: WriteColor> FileNamePrinter<W> {
  pub fn new(writer: W) -> Self {
    Self {
      writer,
      styles: PrintStyles::from(ColorChoice::Auto),
    }
  }

  pub fn color<C: Into<ColorChoice>>(mut self, color: C) -> Self {
    let color = color.into();
    self.styles = PrintStyles::from(color);
    self
  }
}

impl<W: WriteColor> Printer for FileNamePrinter<W> {
  type Processed = Buffer;
  type Processor = FileNameProcessor;

  fn get_processor(&self) -> Self::Processor {
    let color = self.writer.supports_color();
    FileNameProcessor {
      color,
      styles: self.styles.clone(),
    }
  }

  fn process(&mut self, buffer: Buffer) -> Result<()> {
    self.writer.write_all(buffer.as_slice())?;
    Ok(())
  }
}

impl FileNamePrinter<StandardStream> {
  pub fn stdout<C: Into<ColorChoice>>(color: C) -> Self {
    let color = color.into();
    FileNamePrinter::new(StandardStream::stdout(color)).color(color)
  }
}

fn create_buffer(color: bool) -> Buffer {
  if color {
    Buffer::ansi()
  } else {
    Buffer::no_color()
  }
}

pub struct FileNameProcessor {
  color: bool,
  styles: PrintStyles,
}

impl FileNameProcessor {
  fn print_path(&self, path: &Path) -> Result<Buffer> {
    let styles = &self.styles;
    let mut buffer = create_buffer(self.color);
    styles.print_prelude(path, &mut buffer)?;
    Ok(buffer)
  }
}

impl PrintProcessor<Buffer> for FileNameProcessor {
  fn print_rule(
    &self,
    _matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &str>,
    _rule: &RuleConfig<SgLang>,
  ) -> Result<Buffer> {
    let path = Path::new(file.name().as_ref());
    self.print_path(path)
  }

  fn print_matches(&self, _matches: Vec<NodeMatch>, path: &Path) -> Result<Buffer> {
    self.print_path(path)
  }

  fn print_diffs(&self, _diffs: Vec<Diff>, path: &Path) -> Result<Buffer> {
    self.print_path(path)
  }
  fn print_rule_diffs(
    &self,
    _diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<Buffer> {
    self.print_path(path)
  }
}
