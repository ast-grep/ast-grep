use std::borrow::Cow;

use ast_grep_outline::model::{
  EntryRole, OutlineEntry, OutlineItem, OutlineMember, SourcePosition, SourceRange, SymbolType,
};

use crate::print::JsonStyle;

use super::*;
use crate::outline::{OutlineItems, OutlineView};

fn style(view: OutlineView) -> OutlineStyle {
  OutlineStyle::new(view, false, OutlineItems::All)
}

fn color_style(view: OutlineView) -> OutlineStyle {
  OutlineStyle::new(view, true, OutlineItems::All)
}

fn range(line: usize) -> SourceRange {
  SourceRange {
    byte_offset: 0..0,
    start: SourcePosition { line, column: 0 },
    end: SourcePosition { line, column: 0 },
  }
}

fn entry(
  role: EntryRole,
  symbol_type: SymbolType,
  name: &'static str,
  signature: &'static str,
  line: usize,
) -> OutlineEntry<'static> {
  OutlineEntry {
    role,
    symbol_type,
    name: Cow::Borrowed(name),
    range: range(line),
    signature: Cow::Borrowed(signature),
    ast_kind: Cow::Borrowed("test_node"),
  }
}

fn member(
  symbol_type: SymbolType,
  name: &'static str,
  signature: &'static str,
  line: usize,
  is_public: bool,
) -> OutlineMember<'static> {
  OutlineMember {
    entry: entry(EntryRole::Member, symbol_type, name, signature, line),
    is_public,
  }
}

fn outline_file() -> OutlineFile<'static> {
  OutlineFile {
    path: "src/parser.ts".to_string(),
    language: "TypeScript".to_string(),
    items: vec![OutlineItem {
      entry: entry(
        EntryRole::Item,
        SymbolType::Class,
        "Parser",
        "export class Parser",
        39,
      ),
      is_import: false,
      is_exported: true,
      members: vec![
        member(SymbolType::Method, "parse", "parse(...)", 43, true),
        member(SymbolType::Method, "recover", "recover(...)", 72, false),
      ],
    }],
  }
}

#[test]
fn renders_digest_like_design_doc() {
  let mut output = vec![];
  print_text_to(&mut output, &[outline_file()], &style(OutlineView::Digest))
    .expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert_eq!(
    output,
    "src/parser.ts\n40: export class Parser\n      method: parse, recover\n"
  );
}

#[test]
fn renders_expanded_members_like_design_doc() {
  let mut output = vec![];
  print_text_to(
    &mut output,
    &[outline_file()],
    &style(OutlineView::Expanded),
  )
  .expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert_eq!(
    output,
    "src/parser.ts\n40: export class Parser\n44:   parse(...)\n73:   recover(...)\n"
  );
}

#[test]
fn aligns_line_numbers_to_file_width() {
  let mut file = outline_file();
  file.items.push(OutlineItem {
    entry: entry(EntryRole::Item, SymbolType::Function, "late", "late()", 99),
    is_import: false,
    is_exported: false,
    members: vec![],
  });
  let mut output = vec![];
  print_text_to(&mut output, &[file], &style(OutlineView::Expanded)).expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert_eq!(
    output,
    "src/parser.ts\n 40: export class Parser\n 44:   parse(...)\n 73:   recover(...)\n100: late()\n"
  );

  let mut file = outline_file();
  file.items.push(OutlineItem {
    entry: entry(EntryRole::Item, SymbolType::Function, "late", "late()", 99),
    is_import: false,
    is_exported: false,
    members: vec![],
  });
  let mut output = vec![];
  print_text_to(&mut output, &[file], &style(OutlineView::Digest)).expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert!(output.contains("\n       method: parse, recover\n"));
}

#[test]
fn renders_empty_direct_file_block() {
  let mut output = vec![];
  let file = OutlineFile {
    path: "src/empty.ts".to_string(),
    language: "TypeScript".to_string(),
    items: vec![],
  };
  print_text_to(&mut output, &[file], &style(OutlineView::Digest)).expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert_eq!(output, "src/empty.ts\nnothing found\n");
}

#[test]
fn separates_file_blocks_with_blank_line() {
  let mut output = vec![];
  let first = outline_file();
  let mut second = outline_file();
  second.path = "src/checker.ts".to_string();
  print_text_to(
    &mut output,
    &[first, second],
    &style(OutlineView::Signatures),
  )
  .expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert!(output.contains("src/parser.ts\n40: export class Parser\n\nsrc/checker.ts\n"));
}

#[test]
fn emitter_streams_text_file_blocks() {
  let style = style(OutlineView::Signatures);
  let mut output = vec![];
  {
    let mut emitter = OutlineEmitter::new(&mut output, None, style);
    emitter.emit(outline_file()).expect("file should emit");
    let mut second = outline_file();
    second.path = "src/checker.ts".to_string();
    emitter.emit(second).expect("file should emit");
    emitter.finish().expect("output should finish");
  }
  let output = String::from_utf8(output).expect("output should be utf8");

  assert!(output.contains("src/parser.ts\n40: export class Parser\n\nsrc/checker.ts\n"));
}

#[test]
fn emitter_streams_json_lines_per_file() {
  let style = style(OutlineView::Signatures);
  let mut output = vec![];
  {
    let mut emitter = OutlineEmitter::new(&mut output, Some(JsonStyle::Stream), style);
    emitter.emit(outline_file()).expect("file should emit");
    let mut second = outline_file();
    second.path = "src/checker.ts".to_string();
    emitter.emit(second).expect("file should emit");
    emitter.finish().expect("output should finish");
  }
  let output = String::from_utf8(output).expect("output should be utf8");
  let lines = output.lines().collect::<Vec<_>>();

  assert_eq!(lines.len(), 2);
  for line in lines {
    serde_json::from_str::<serde_json::Value>(line).expect("line should be json");
  }
}

#[test]
fn emitter_streams_valid_compact_json_array() {
  let style = style(OutlineView::Signatures);
  let mut output = vec![];
  {
    let mut emitter = OutlineEmitter::new(&mut output, Some(JsonStyle::Compact), style);
    emitter.emit(outline_file()).expect("file should emit");
    let mut second = outline_file();
    second.path = "src/checker.ts".to_string();
    emitter.emit(second).expect("file should emit");
    emitter.finish().expect("output should finish");
  }
  let output: serde_json::Value =
    serde_json::from_slice(&output).expect("output should be a json array");

  assert_eq!(output.as_array().expect("json should be array").len(), 2);
}

#[test]
fn signature_view_styles_exported_items() {
  let style = color_style(OutlineView::Signatures);
  let mut output = vec![];
  print_text_to(&mut output, &[outline_file()], &style).expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert!(output.contains("export class \u{1b}[1;34mParser\u{1b}[0m"));
}

#[test]
fn colors_symbol_types_differently() {
  let style = OutlineTextStyle::new(true, OutlineItems::All);
  let class = style.grouped_label(SymbolType::Class, "class");
  let function = style.grouped_label(SymbolType::Function, "function");

  assert_ne!(class, function);
  assert!(class.contains("\u{1b}["));
  assert!(function.contains("\u{1b}["));
  assert!(!function.contains("\u{1b}[7;"));
  assert!(function.contains("function"));
}

#[test]
fn styles_outline_flags_with_ansi() {
  let style = OutlineTextStyle::new(true, OutlineItems::All);
  let file = style.file("src/parser.ts");
  let import = style.entry_signature(
    &entry(
      EntryRole::Item,
      SymbolType::Module,
      "std::fmt",
      "use std::fmt;",
      0,
    ),
    SymbolType::Module,
    true,
    false,
  );
  let exported = style.entry_signature(
    &entry(
      EntryRole::Item,
      SymbolType::Function,
      "parse",
      "pub fn parse()",
      0,
    ),
    SymbolType::Function,
    false,
    true,
  );
  let public_member = style.member_signature(
    &member(SymbolType::Method, "parse", "parse()", 0, true),
    SymbolType::Method,
  );
  let private_member = style.member_signature(
    &member(SymbolType::Method, "recover", "recover()", 0, false),
    SymbolType::Method,
  );

  assert!(file.contains("\u{1b}[4"));
  assert!(import.contains("\u{1b}["));
  assert!(exported.contains("\u{1b}["));
  assert_ne!(import, "use std::fmt;");
  assert_ne!(exported, "pub fn parse()");
  assert_ne!(public_member, private_member);
  assert!(private_member.contains("\u{1b}["));
}

#[test]
fn keeps_digest_and_names_entries_uncolored() {
  let style = color_style(OutlineView::Names);
  let mut output = vec![];
  print_text_to(&mut output, &[outline_file()], &style).expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert!(output.contains("Parser"));
  assert!(!output.contains("\u{1b}[34mParser"));
  assert!(output.contains("\u{1b}[1mParser"));

  let style = color_style(OutlineView::Digest);
  let mut output = vec![];
  print_text_to(&mut output, &[outline_file()], &style).expect("text should render");
  let output = String::from_utf8(output).expect("output should be utf8");

  assert!(output.contains("parse"));
  assert!(output.contains("recover"));
  assert!(!output.contains("\u{1b}[32mparse"));
  assert!(!output.contains("\u{1b}[2;32mrecover"));
  assert!(output.contains("\u{1b}[2mrecover"));
}

#[test]
fn suppresses_redundant_item_mode_styles() {
  let import_style = OutlineTextStyle::new(true, OutlineItems::Imports);
  let export_style = OutlineTextStyle::new(true, OutlineItems::Exports);
  let mixed_style = OutlineTextStyle::new(true, OutlineItems::All);
  let import_name = StyledName {
    text: "std::fmt".to_string(),
    is_import: true,
    is_exported: false,
    is_public: true,
  };
  let export_name = StyledName {
    text: "api".to_string(),
    is_import: false,
    is_exported: true,
    is_public: true,
  };

  assert_eq!(import_style.grouped_item_name(&import_name), "std::fmt");
  assert_eq!(export_style.grouped_item_name(&export_name), "api");
  assert_ne!(mixed_style.grouped_item_name(&import_name), "std::fmt");
  assert_ne!(mixed_style.grouped_item_name(&export_name), "api");

  let import_signature = entry(
    EntryRole::Item,
    SymbolType::Module,
    "std::fmt",
    "use std::fmt;",
    0,
  );
  let export_signature = entry(
    EntryRole::Item,
    SymbolType::Function,
    "api",
    "pub fn api()",
    0,
  );

  assert_eq!(
    import_style.entry_signature(&import_signature, SymbolType::Module, true, false),
    "use \u{1b}[36mstd::fmt\u{1b}[0m;"
  );
  assert_eq!(
    export_style.entry_signature(&export_signature, SymbolType::Function, false, true),
    "pub fn \u{1b}[32mapi\u{1b}[0m()"
  );
}
