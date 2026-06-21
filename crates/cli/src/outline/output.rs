use std::collections::HashMap;
use std::fmt::Display;
use std::io::Write;

use anyhow::Result;
use ast_grep_outline::model::{OutlineEntry, OutlineItem, OutlineMember, SymbolType};

use crate::print::JsonStyle;

use super::extract::OutlineFile;
use super::options::{has_directory_input, resolve_items, resolve_view};
use super::{OutlineArg, OutlineItems, OutlineView};

#[cfg(test)]
mod tests;

pub struct OutlineStyle {
  view: OutlineView,
  text: OutlineTextStyle,
}

impl OutlineStyle {
  pub fn from_arg(arg: &OutlineArg) -> Self {
    let has_directory_input = has_directory_input(arg);
    let items = resolve_items(arg.items, has_directory_input);
    let view = resolve_view(arg.view, has_directory_input);
    Self::new(view, arg.color.should_use_color(), items)
  }

  fn new(view: OutlineView, use_color: bool, items: OutlineItems) -> Self {
    Self {
      view,
      text: OutlineTextStyle::new(use_color, items),
    }
  }
}

pub struct OutlineEmitter<W> {
  out: W,
  json: Option<JsonStyle>,
  style: OutlineStyle,
  is_first: bool,
  emitted_any: bool,
}

impl<W: Write> OutlineEmitter<W> {
  pub fn new(out: W, json: Option<JsonStyle>, style: OutlineStyle) -> Self {
    Self {
      out,
      json,
      style,
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
      None => print_text_file_to(&mut self.out, &file, &self.style, self.is_first)?,
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
  style: &OutlineStyle,
) -> Result<()> {
  if files.is_empty() {
    writeln!(out, "nothing found")?;
    return Ok(());
  }
  for (idx, file) in files.iter().enumerate() {
    print_text_file_to(&mut out, file, style, idx == 0)?;
  }
  Ok(())
}

fn print_text_file_to(
  mut out: &mut impl Write,
  file: &OutlineFile,
  style: &OutlineStyle,
  is_first: bool,
) -> Result<()> {
  if !is_first {
    writeln!(out)?;
  }
  let text_style = &style.text;
  writeln!(out, "{}", text_style.file(&file.path))?;
  if file.items.is_empty() {
    writeln!(out, "nothing found")?;
  } else {
    let line_number_width = line_number_width(file);
    match style.view {
      OutlineView::Auto => unreachable!("outline view should be resolved"),
      OutlineView::Names => print_names(&mut out, file, text_style)?,
      OutlineView::Signatures => print_signatures(&mut out, file, text_style, line_number_width)?,
      OutlineView::Digest => print_digest(&mut out, file, text_style, line_number_width)?,
      OutlineView::Expanded => print_expanded(&mut out, file, text_style, line_number_width)?,
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
struct StyledName<'a> {
  text: &'a str,
  overload_count: usize,
  is_import: bool,
  is_exported: bool,
  is_public: bool,
}

fn grouped_item_names<'a>(items: &'a [OutlineItem]) -> Vec<(SymbolType, Vec<StyledName<'a>>)> {
  let mut groups = GroupedNames::new();
  for item in items {
    groups.push(
      item.entry.symbol_type,
      StyledName {
        text: item.entry.name.as_ref(),
        overload_count: 1,
        is_import: item.is_import,
        is_exported: item.is_exported,
        is_public: true,
      },
    );
  }
  groups.into_vec()
}

fn grouped_member_names<'a>(
  members: &'a [OutlineMember],
) -> Vec<(SymbolType, Vec<StyledName<'a>>)> {
  let mut groups = GroupedNames::new();
  for member in members.iter().filter(|member| member.is_public) {
    groups.push(
      member.entry.symbol_type,
      StyledName {
        text: member.entry.name.as_ref(),
        overload_count: 1,
        is_import: false,
        is_exported: false,
        is_public: member.is_public,
      },
    );
  }
  for member in members.iter().filter(|member| !member.is_public) {
    groups.push(
      member.entry.symbol_type,
      StyledName {
        text: member.entry.name.as_ref(),
        overload_count: 1,
        is_import: false,
        is_exported: false,
        is_public: member.is_public,
      },
    );
  }
  groups.into_vec()
}

struct GroupedNames<'a> {
  groups: Vec<GroupedNameBucket<'a>>,
}

impl<'a> GroupedNames<'a> {
  fn new() -> Self {
    Self {
      groups: SYMBOL_TYPE_ORDER
        .iter()
        .map(|&symbol_type| GroupedNameBucket::new(symbol_type))
        .collect(),
    }
  }

  fn push(&mut self, symbol_type: SymbolType, name: StyledName<'a>) {
    let group_index = match self
      .groups
      .iter()
      .position(|group| group.symbol_type == symbol_type)
    {
      Some(index) => index,
      None => {
        self.groups.push(GroupedNameBucket::new(symbol_type));
        self.groups.len() - 1
      }
    };
    self.groups[group_index].push(name);
  }

  fn into_vec(self) -> Vec<(SymbolType, Vec<StyledName<'a>>)> {
    self
      .groups
      .into_iter()
      .filter_map(|group| {
        if group.names.is_empty() {
          None
        } else {
          Some((group.symbol_type, group.names))
        }
      })
      .collect()
  }
}

struct GroupedNameBucket<'a> {
  symbol_type: SymbolType,
  names: Vec<StyledName<'a>>,
  name_indices: HashMap<&'a str, usize>,
}

impl<'a> GroupedNameBucket<'a> {
  fn new(symbol_type: SymbolType) -> Self {
    Self {
      symbol_type,
      names: vec![],
      name_indices: HashMap::new(),
    }
  }

  fn push(&mut self, name: StyledName<'a>) {
    if let Some(&index) = self.name_indices.get(name.text) {
      let existing = &mut self.names[index];
      existing.overload_count += 1;
      existing.is_import |= name.is_import;
      existing.is_exported |= name.is_exported;
      existing.is_public |= name.is_public;
      return;
    }

    let index = self.names.len();
    self.name_indices.insert(name.text, index);
    self.names.push(name);
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
    self.grouped_name(name, style)
  }

  fn grouped_member_name(&self, name: &StyledName) -> String {
    let style = if name.is_public {
      ansi_term::Style::new()
    } else {
      ansi_term::Style::new().dimmed()
    };
    self.grouped_name(name, style)
  }

  fn grouped_name(&self, name: &StyledName, name_style: ansi_term::Style) -> String {
    let text = self.paint(name_style, name.text);
    if name.overload_count == 1 {
      return text;
    }
    format!("{text} ×{}", name.overload_count)
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
