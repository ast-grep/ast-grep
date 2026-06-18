use anyhow::{Context, Result};
use ast_grep_outline::model::{OutlineItem, SymbolType};
use regex::Regex;

use super::{OutlineArg, OutlineItems, OutlineView};

#[derive(Clone)]
pub(super) struct OutlineTextOptions {
  pub(super) items: OutlineItems,
  pub(super) view: OutlineView,
  pub(super) symbol_types: Option<Vec<SymbolType>>,
  pub(super) item_matcher: Option<Regex>,
  pub(super) pub_members: bool,
  pub(super) use_color: bool,
  pub(super) show_empty_files: bool,
}

impl OutlineTextOptions {
  pub(super) fn try_from_arg(arg: &OutlineArg) -> Result<Self> {
    let has_directory_input = !arg.input.stdin && arg.input.paths.iter().any(|path| path.is_dir());
    Ok(Self {
      items: resolve_items(arg.items, has_directory_input),
      view: resolve_view(arg.view, has_directory_input),
      symbol_types: arg
        .symbol_type
        .as_deref()
        .map(parse_symbol_types)
        .transpose()?,
      item_matcher: arg
        .match_item
        .as_deref()
        .map(Regex::new)
        .transpose()
        .context("Cannot parse outline item matcher")?,
      pub_members: arg.pub_members,
      use_color: arg.color.should_use_color(),
      show_empty_files: arg.input.stdin || !has_directory_input,
    })
  }
}

fn resolve_items(items: OutlineItems, has_directory_input: bool) -> OutlineItems {
  match (items, has_directory_input) {
    (OutlineItems::Auto, true) => OutlineItems::Exports,
    (OutlineItems::Auto, false) => OutlineItems::Structure,
    _ => items,
  }
}

fn resolve_view(view: OutlineView, has_directory_input: bool) -> OutlineView {
  match (view, has_directory_input) {
    (OutlineView::Auto, true) => OutlineView::Names,
    (OutlineView::Auto, false) => OutlineView::Digest,
    _ => view,
  }
}

fn parse_symbol_types(source: &str) -> Result<Vec<SymbolType>> {
  source
    .split(',')
    .map(|raw| {
      let raw = raw.trim();
      parse_symbol_type(raw).with_context(|| format!("Unknown outline symbol type `{raw}`"))
    })
    .collect()
}

fn parse_symbol_type(raw: &str) -> Option<SymbolType> {
  let normalized = raw
    .chars()
    .filter(|ch| *ch != '-' && *ch != '_')
    .flat_map(char::to_lowercase)
    .collect::<String>();
  Some(match normalized.as_str() {
    "file" => SymbolType::File,
    "module" => SymbolType::Module,
    "namespace" => SymbolType::Namespace,
    "package" => SymbolType::Package,
    "class" => SymbolType::Class,
    "method" => SymbolType::Method,
    "property" => SymbolType::Property,
    "field" => SymbolType::Field,
    "constructor" => SymbolType::Constructor,
    "enum" => SymbolType::Enum,
    "interface" => SymbolType::Interface,
    "function" => SymbolType::Function,
    "variable" => SymbolType::Variable,
    "constant" => SymbolType::Constant,
    "string" => SymbolType::String,
    "number" => SymbolType::Number,
    "boolean" => SymbolType::Boolean,
    "array" => SymbolType::Array,
    "object" => SymbolType::Object,
    "key" => SymbolType::Key,
    "null" => SymbolType::Null,
    "enummember" => SymbolType::EnumMember,
    "struct" => SymbolType::Struct,
    "event" => SymbolType::Event,
    "operator" => SymbolType::Operator,
    "typeparameter" => SymbolType::TypeParameter,
    _ => return None,
  })
}

pub(super) fn matches_item_filters(item: &OutlineItem, options: &OutlineTextOptions) -> bool {
  matches_items_mode(item, options.items)
    && matches_symbol_type(item, options.symbol_types.as_deref())
    && matches_item_regex(item, options.item_matcher.as_ref())
}

fn matches_items_mode(item: &OutlineItem, items: OutlineItems) -> bool {
  match items {
    OutlineItems::Auto => unreachable!("outline item mode should be resolved"),
    OutlineItems::Structure => !item.is_import,
    OutlineItems::Exports => item.is_exported,
    OutlineItems::Imports => item.is_import,
    OutlineItems::All => true,
  }
}

fn matches_symbol_type(item: &OutlineItem, symbol_types: Option<&[SymbolType]>) -> bool {
  symbol_types.is_none_or(|types| types.contains(&item.entry.symbol_type))
}

fn matches_item_regex(item: &OutlineItem, matcher: Option<&Regex>) -> bool {
  matcher.is_none_or(|matcher| {
    matcher.is_match(&item.entry.name) || matcher.is_match(&item.entry.signature)
  })
}
