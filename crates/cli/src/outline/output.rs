use std::fmt::Display;
use std::io::Write;

use anyhow::Result;
use ast_grep_outline::model::{OutlineEntry, OutlineItem, OutlineMember, SymbolType};

use crate::print::JsonStyle;

use super::extract::OutlineFile;
use super::options::OutlineTextOptions;
use super::{OutlineItems, OutlineView};

#[cfg(test)]
mod tests;

pub struct OutlineEmitter<'a, W> {
  out: W,
  json: Option<JsonStyle>,
  options: &'a OutlineTextOptions,
  text_style: OutlineTextStyle,
  is_first: bool,
  emitted_any: bool,
}

impl<'a, W: Write> OutlineEmitter<'a, W> {
  pub fn new(out: W, json: Option<JsonStyle>, options: &'a OutlineTextOptions) -> Self {
    Self {
      out,
      json,
      options,
      text_style: OutlineTextStyle::new(options.use_color, options.items),
      is_first: true,
      emitted_any: false,
    }
  }

  pub fn emit(&mut self, file: OutlineFile<'static>) -> Result<()> {
    match self.json {
      Some(JsonStyle::Pretty) => self.emit_pretty_json(&file)?,
      Some(JsonStyle::Compact) => self.emit_compact_json(&file)?,
      Some(JsonStyle::Stream) => {
        serde_json::to_writer(&mut self.out, &file)?;
        writeln!(self.out)?;
      }
      None => print_text_file_to(
        &mut self.out,
        &file,
        self.options,
        &self.text_style,
        self.is_first,
      )?,
    }
    self.is_first = false;
    self.emitted_any = true;
    self.out.flush()?;
    Ok(())
  }

  pub fn finish(&mut self) -> Result<()> {
    match self.json {
      Some(JsonStyle::Pretty) => {
        if self.emitted_any {
          writeln!(self.out, "]")?;
        } else {
          writeln!(self.out, "[]")?;
        }
      }
      Some(JsonStyle::Compact) => {
        if self.emitted_any {
          writeln!(self.out, "]")?;
        } else {
          writeln!(self.out, "[]")?;
        }
      }
      Some(JsonStyle::Stream) => {}
      None if !self.emitted_any => {
        writeln!(self.out, "nothing found")?;
      }
      None => {}
    }
    self.out.flush()?;
    Ok(())
  }

  fn emit_pretty_json(&mut self, file: &OutlineFile) -> Result<()> {
    if self.is_first {
      writeln!(self.out, "[")?;
    } else {
      writeln!(self.out, ",")?;
    }
    let object = serde_json::to_string_pretty(file)?;
    for line in object.lines() {
      writeln!(self.out, "  {line}")?;
    }
    Ok(())
  }

  fn emit_compact_json(&mut self, file: &OutlineFile) -> Result<()> {
    if self.is_first {
      write!(self.out, "[")?;
    } else {
      write!(self.out, ",")?;
    }
    serde_json::to_writer(&mut self.out, file)?;
    Ok(())
  }
}

#[cfg(test)]
fn print_text_to(
  mut out: &mut impl Write,
  files: &[OutlineFile],
  options: &OutlineTextOptions,
) -> Result<()> {
  let style = OutlineTextStyle::new(options.use_color, options.items);
  if files.is_empty() {
    writeln!(out, "nothing found")?;
    return Ok(());
  }
  for (idx, file) in files.iter().enumerate() {
    print_text_file_to(&mut out, file, options, &style, idx == 0)?;
  }
  Ok(())
}

fn print_text_file_to(
  mut out: &mut impl Write,
  file: &OutlineFile,
  options: &OutlineTextOptions,
  style: &OutlineTextStyle,
  is_first: bool,
) -> Result<()> {
  if !is_first {
    writeln!(out)?;
  }
  writeln!(out, "{}", style.file(&file.path))?;
  if file.items.is_empty() {
    writeln!(out, "nothing found")?;
  } else {
    let line_number_width = line_number_width(file);
    match options.view {
      OutlineView::Auto => unreachable!("outline view should be resolved"),
      OutlineView::Names => print_names(&mut out, file, style)?,
      OutlineView::Signatures => print_signatures(&mut out, file, style, line_number_width)?,
      OutlineView::Digest => print_digest(&mut out, file, style, line_number_width)?,
      OutlineView::Expanded => print_expanded(&mut out, file, style, line_number_width)?,
    }
  }
  Ok(())
}

fn print_names(out: &mut impl Write, file: &OutlineFile, style: &OutlineTextStyle) -> Result<()> {
  for (symbol_type, names) in grouped_item_names(&file.items) {
    writeln!(
      out,
      "{}: {}",
      style.grouped_label(symbol_type, symbol_type_name(symbol_type)),
      names
        .iter()
        .map(|name| style.grouped_item_name(name))
        .collect::<Vec<_>>()
        .join(", ")
    )?;
  }
  Ok(())
}

fn print_signatures(
  out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> Result<()> {
  for item in &file.items {
    writeln!(out, "{}", item_line(item, style, true, line_number_width))?;
  }
  Ok(())
}

fn print_digest(
  out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> Result<()> {
  let member_indent = grouped_member_indent(line_number_width);
  for item in &file.items {
    writeln!(out, "{}", item_line(item, style, true, line_number_width))?;
    for (symbol_type, names) in grouped_member_names(&item.members) {
      writeln!(
        out,
        "{}{}: {}",
        member_indent,
        style.grouped_label(symbol_type, symbol_type_name(symbol_type)),
        names
          .iter()
          .map(|name| style.grouped_member_name(name))
          .collect::<Vec<_>>()
          .join(", ")
      )?;
    }
  }
  Ok(())
}

fn grouped_member_indent(line_number_width: usize) -> String {
  " ".repeat(line_number_width + 4)
}

fn print_expanded(
  out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> Result<()> {
  for item in &file.items {
    writeln!(out, "{}", item_line(item, style, true, line_number_width))?;
    for member in &item.members {
      writeln!(out, "{}", member_line(member, style, line_number_width))?;
    }
  }
  Ok(())
}

fn line_number_width(file: &OutlineFile) -> usize {
  file
    .items
    .iter()
    .flat_map(|item| {
      std::iter::once(item.entry.range.start.line + 1).chain(
        item
          .members
          .iter()
          .map(|member| member.entry.range.start.line + 1),
      )
    })
    .max()
    .unwrap_or(1)
    .to_string()
    .len()
}

#[derive(Clone)]
struct StyledName {
  text: String,
  is_import: bool,
  is_exported: bool,
  is_public: bool,
}

fn grouped_item_names(items: &[OutlineItem]) -> Vec<(SymbolType, Vec<StyledName>)> {
  let mut groups = empty_symbol_groups();
  for item in items {
    push_grouped_name(
      &mut groups,
      item.entry.symbol_type,
      StyledName {
        text: item.entry.name.to_string(),
        is_import: item.is_import,
        is_exported: item.is_exported,
        is_public: true,
      },
    );
  }
  groups.retain(|(_, names)| !names.is_empty());
  groups
}

fn grouped_member_names(members: &[OutlineMember]) -> Vec<(SymbolType, Vec<StyledName>)> {
  let mut groups = empty_symbol_groups();
  for member in members.iter().filter(|member| member.is_public) {
    push_grouped_name(
      &mut groups,
      member.entry.symbol_type,
      StyledName {
        text: member.entry.name.to_string(),
        is_import: false,
        is_exported: false,
        is_public: member.is_public,
      },
    );
  }
  for member in members.iter().filter(|member| !member.is_public) {
    push_grouped_name(
      &mut groups,
      member.entry.symbol_type,
      StyledName {
        text: member.entry.name.to_string(),
        is_import: false,
        is_exported: false,
        is_public: member.is_public,
      },
    );
  }
  groups.retain(|(_, names)| !names.is_empty());
  groups
}

fn empty_symbol_groups() -> Vec<(SymbolType, Vec<StyledName>)> {
  SYMBOL_TYPE_ORDER
    .iter()
    .map(|&symbol_type| (symbol_type, vec![]))
    .collect()
}

fn push_grouped_name(
  groups: &mut Vec<(SymbolType, Vec<StyledName>)>,
  symbol_type: SymbolType,
  name: StyledName,
) {
  if let Some((_, names)) = groups.iter_mut().find(|(ty, _)| *ty == symbol_type) {
    names.push(name);
  } else {
    groups.push((symbol_type, vec![name]));
  }
}

fn item_line(
  item: &OutlineItem,
  style: &OutlineTextStyle,
  emphasize_exported: bool,
  line_number_width: usize,
) -> String {
  format!(
    "{}: {}",
    style.line_number(format_args!(
      "{:>line_number_width$}",
      item.entry.range.start.line + 1
    )),
    style.entry_signature(
      &item.entry,
      item.entry.symbol_type,
      item.is_import,
      emphasize_exported && item.is_exported
    )
  )
}

fn member_line(
  member: &OutlineMember,
  style: &OutlineTextStyle,
  line_number_width: usize,
) -> String {
  format!(
    "{}:   {}",
    style.line_number(format_args!(
      "{:>line_number_width$}",
      member.entry.range.start.line + 1
    )),
    style.member_signature(member, member.entry.symbol_type)
  )
}

fn signature_or_name<'entry, 'text>(entry: &'entry OutlineEntry<'text>) -> &'entry str {
  if entry.signature.is_empty() {
    &entry.name
  } else {
    &entry.signature
  }
}

fn symbol_type_name(symbol_type: SymbolType) -> &'static str {
  match symbol_type {
    SymbolType::File => "file",
    SymbolType::Module => "module",
    SymbolType::Namespace => "namespace",
    SymbolType::Package => "package",
    SymbolType::Class => "class",
    SymbolType::Method => "method",
    SymbolType::Property => "property",
    SymbolType::Field => "field",
    SymbolType::Constructor => "constructor",
    SymbolType::Enum => "enum",
    SymbolType::Interface => "interface",
    SymbolType::Function => "function",
    SymbolType::Variable => "variable",
    SymbolType::Constant => "constant",
    SymbolType::String => "string",
    SymbolType::Number => "number",
    SymbolType::Boolean => "boolean",
    SymbolType::Array => "array",
    SymbolType::Object => "object",
    SymbolType::Key => "key",
    SymbolType::Null => "null",
    SymbolType::EnumMember => "enumMember",
    SymbolType::Struct => "struct",
    SymbolType::Event => "event",
    SymbolType::Operator => "operator",
    SymbolType::TypeParameter => "typeParameter",
  }
}

const SYMBOL_TYPE_ORDER: &[SymbolType] = &[
  SymbolType::File,
  SymbolType::Module,
  SymbolType::Namespace,
  SymbolType::Package,
  SymbolType::Class,
  SymbolType::Struct,
  SymbolType::Enum,
  SymbolType::Interface,
  SymbolType::Function,
  SymbolType::Method,
  SymbolType::Constructor,
  SymbolType::Property,
  SymbolType::Field,
  SymbolType::EnumMember,
  SymbolType::Variable,
  SymbolType::Constant,
  SymbolType::String,
  SymbolType::Number,
  SymbolType::Boolean,
  SymbolType::Array,
  SymbolType::Object,
  SymbolType::Key,
  SymbolType::Null,
  SymbolType::Event,
  SymbolType::Operator,
  SymbolType::TypeParameter,
];

struct OutlineTextStyle {
  use_color: bool,
  emphasize_imports: bool,
  emphasize_exports: bool,
}

impl OutlineTextStyle {
  fn new(use_color: bool, items: OutlineItems) -> Self {
    Self {
      use_color,
      emphasize_imports: items != OutlineItems::Imports,
      emphasize_exports: items != OutlineItems::Exports,
    }
  }

  fn file(&self, text: impl Display) -> String {
    self.paint(ansi_term::Style::new().underline(), text)
  }

  fn line_number(&self, text: impl Display) -> String {
    self.paint(ansi_term::Style::new().dimmed(), text)
  }

  fn grouped_label(&self, symbol_type: SymbolType, text: impl Display) -> String {
    let text = text.to_string();
    if !self.use_color {
      return text;
    }
    symbol_type_style(symbol_type).paint(text).to_string()
  }

  fn entry_signature(
    &self,
    entry: &OutlineEntry,
    symbol_type: SymbolType,
    is_import: bool,
    emphasize_name: bool,
  ) -> String {
    let name_style = item_name_style(
      symbol_type,
      self.emphasize_imports && is_import,
      self.emphasize_exports && emphasize_name,
    );
    self.signature(entry, name_style, None)
  }

  fn member_signature(&self, member: &OutlineMember, symbol_type: SymbolType) -> String {
    let surrounding_style = if member.is_public {
      None
    } else {
      Some(ansi_term::Style::new().dimmed())
    };
    let name_style = if member.is_public {
      symbol_type_style(symbol_type)
    } else {
      symbol_type_style(symbol_type).dimmed()
    };
    self.signature(&member.entry, name_style, surrounding_style)
  }

  fn grouped_item_name(&self, name: &StyledName) -> String {
    let mut style = ansi_term::Style::new();
    if self.emphasize_imports && name.is_import {
      style = style.italic();
    }
    if self.emphasize_exports && name.is_exported {
      style = style.bold();
    }
    self.paint(style, &name.text)
  }

  fn grouped_member_name(&self, name: &StyledName) -> String {
    let style = if name.is_public {
      ansi_term::Style::new()
    } else {
      ansi_term::Style::new().dimmed()
    };
    self.paint(style, &name.text)
  }

  fn paint(&self, style: ansi_term::Style, text: impl Display) -> String {
    if self.use_color {
      style.paint(text.to_string()).to_string()
    } else {
      text.to_string()
    }
  }

  fn signature(
    &self,
    entry: &OutlineEntry,
    name_style: ansi_term::Style,
    surrounding_style: Option<ansi_term::Style>,
  ) -> String {
    let signature = signature_or_name(entry);
    if !self.use_color {
      return signature.to_string();
    }
    let Some(start) = signature.find(entry.name.as_ref()) else {
      return surrounding_style.map_or_else(
        || signature.to_string(),
        |style| self.paint(style, signature),
      );
    };
    let end = start + entry.name.len();
    let before = surrounding_style.map_or_else(
      || signature[..start].to_string(),
      |style| self.paint(style, &signature[..start]),
    );
    let name = name_style.paint(&signature[start..end]).to_string();
    let after = surrounding_style.map_or_else(
      || signature[end..].to_string(),
      |style| self.paint(style, &signature[end..]),
    );
    format!("{before}{name}{after}")
  }
}

fn symbol_type_style(symbol_type: SymbolType) -> ansi_term::Style {
  use ansi_term::Color;
  let color = match symbol_type {
    SymbolType::File | SymbolType::Module | SymbolType::Namespace | SymbolType::Package => {
      Color::Cyan
    }
    SymbolType::Class | SymbolType::Struct | SymbolType::Object => Color::Blue,
    SymbolType::Enum | SymbolType::EnumMember => Color::Purple,
    SymbolType::Interface | SymbolType::TypeParameter => Color::Red,
    SymbolType::Function | SymbolType::Method | SymbolType::Constructor => Color::Green,
    SymbolType::Property | SymbolType::Field | SymbolType::Key => Color::Yellow,
    SymbolType::Variable | SymbolType::Constant => Color::Fixed(214),
    SymbolType::String
    | SymbolType::Number
    | SymbolType::Boolean
    | SymbolType::Array
    | SymbolType::Null => Color::Fixed(208),
    SymbolType::Event | SymbolType::Operator => Color::Fixed(39),
  };
  color.normal()
}

fn item_name_style(
  symbol_type: SymbolType,
  is_import: bool,
  is_exported: bool,
) -> ansi_term::Style {
  let mut style = symbol_type_style(symbol_type);
  if is_import {
    style = style.italic();
  }
  if is_exported {
    style = style.bold();
  }
  style
}
