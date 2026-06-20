use anyhow::{Context, Result};
use ast_grep_outline::{
  model::SymbolType,
  options::{OutlineEntryDetail, OutlineExtractorOptions, OutlineFlagFilter, OutlineMemberOptions},
};
use regex::Regex;

use super::{OutlineArg, OutlineItems, OutlineView};

pub(super) fn extractor_options_from_arg(arg: &OutlineArg) -> Result<OutlineExtractorOptions> {
  let has_directory_input = has_directory_input(arg);
  let items = resolve_items(arg.items, has_directory_input);
  let view = resolve_view(arg.view, has_directory_input);
  let symbol_types = arg
    .symbol_type
    .as_deref()
    .map(parse_symbol_types)
    .transpose()?;
  let item_regex = arg
    .match_item
    .as_deref()
    .map(Regex::new)
    .transpose()
    .context("Cannot parse outline item matcher")?;
  Ok(extractor_options(
    items,
    view,
    symbol_types,
    item_regex,
    arg.pub_members,
  ))
}

pub(super) fn show_empty_files(arg: &OutlineArg) -> bool {
  arg.input.stdin || !has_directory_input(arg)
}

fn extractor_options(
  items: OutlineItems,
  view: OutlineView,
  symbol_types: Option<Vec<SymbolType>>,
  item_regex: Option<Regex>,
  pub_members: bool,
) -> OutlineExtractorOptions {
  let detail = if item_regex.is_some() {
    OutlineEntryDetail::Signature
  } else {
    match view {
      OutlineView::Auto => unreachable!("outline view should be resolved"),
      OutlineView::Names => OutlineEntryDetail::Name,
      OutlineView::Signatures | OutlineView::Digest | OutlineView::Expanded => {
        OutlineEntryDetail::Signature
      }
    }
  };
  OutlineExtractorOptions {
    symbol_types,
    item_regex,
    imports: match items {
      OutlineItems::Auto => unreachable!("outline item mode should be resolved"),
      OutlineItems::Structure => OutlineFlagFilter::No,
      OutlineItems::Imports => OutlineFlagFilter::Yes,
      OutlineItems::Exports | OutlineItems::All => OutlineFlagFilter::Any,
    },
    exported: match items {
      OutlineItems::Auto => unreachable!("outline item mode should be resolved"),
      OutlineItems::Exports => OutlineFlagFilter::Yes,
      OutlineItems::Structure | OutlineItems::Imports | OutlineItems::All => OutlineFlagFilter::Any,
    },
    detail,
    members: matches!(view, OutlineView::Digest | OutlineView::Expanded).then(|| {
      OutlineMemberOptions {
        public: if pub_members {
          OutlineFlagFilter::Yes
        } else {
          OutlineFlagFilter::Any
        },
        detail: match view {
          OutlineView::Auto => unreachable!("outline view should be resolved"),
          OutlineView::Names | OutlineView::Signatures | OutlineView::Digest => {
            OutlineEntryDetail::Name
          }
          OutlineView::Expanded => OutlineEntryDetail::Signature,
        },
      }
    }),
  }
}

pub(super) fn has_directory_input(arg: &OutlineArg) -> bool {
  !arg.input.stdin && arg.input.paths.iter().any(|path| path.is_dir())
}

pub(super) fn resolve_items(items: OutlineItems, has_directory_input: bool) -> OutlineItems {
  match (items, has_directory_input) {
    (OutlineItems::Auto, true) => OutlineItems::Exports,
    (OutlineItems::Auto, false) => OutlineItems::Structure,
    _ => items,
  }
}

pub(super) fn resolve_view(view: OutlineView, has_directory_input: bool) -> OutlineView {
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
