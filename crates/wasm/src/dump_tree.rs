use ast_grep_core::{matcher::PatternNode, Language, Node, Pattern, StrDoc};
use ast_grep_language::SupportLang;
use serde::{Deserialize, Serialize};
use tree_sitter as ts;
use wasm_bindgen::prelude::JsError;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpNode {
  id: usize,
  field: Option<String>,
  kind: String,
  start: Pos,
  end: Pos,
  is_named: bool,
  children: Vec<DumpNode>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pos {
  row: u32,
  column: u32,
}

impl From<ts::Point> for Pos {
  #[inline]
  fn from(pt: ts::Point) -> Self {
    Pos {
      row: pt.row as u32,
      column: pt.column as u32,
    }
  }
}

pub fn dump_one_node(cursor: &mut ts::TreeCursor, target: &mut Vec<DumpNode>) {
  let node = cursor.node();
  let kind = if node.is_missing() {
    format!("MISSING {}", node.kind())
  } else {
    node.kind().to_string()
  };
  let start = node.start_position().into();
  let end = node.end_position().into();
  let field = cursor.field_name().map(|c| c.to_string());
  let mut children = vec![];
  if cursor.goto_first_child() {
    dump_nodes(cursor, &mut children);
    cursor.goto_parent();
  }
  target.push(DumpNode {
    id: node.id(),
    field,
    kind,
    start,
    end,
    children,
    is_named: node.is_named(),
  })
}

fn dump_nodes(cursor: &mut ts::TreeCursor, target: &mut Vec<DumpNode>) {
  loop {
    dump_one_node(cursor, target);
    if !cursor.goto_next_sibling() {
      break;
    }
  }
}

pub fn dump_pattern(
  lang: SupportLang,
  query: String,
  selector: Option<String>,
) -> Result<PatternTree, JsError> {
  let processed = lang.pre_process_pattern(&query);
  let root = lang.ast_grep(processed);
  let pattern = if let Some(sel) = selector {
    Pattern::contextual(&query, &sel, lang)?
  } else {
    Pattern::try_new(&query, lang)?
  };
  let found = root
    .root()
    .find(&pattern)
    .ok_or_else(|| JsError::new("pattern node not found"))?;
  let ret = dump_pattern_tree(root.root(), found.node_id(), &pattern.node);
  Ok(ret)
}

fn dump_pattern_tree(
  node: Node<StrDoc<SupportLang>>,
  node_id: usize,
  pattern: &PatternNode,
) -> PatternTree {
  if node.node_id() == node_id {
    return dump_pattern_impl(node, pattern);
  }
  let children: Vec<_> = node
    .children()
    .map(|n| dump_pattern_tree(n, node_id, pattern))
    .collect();
  let ts = node.get_ts_node();
  let text = if children.is_empty() {
    Some(node.text().into())
  } else {
    None
  };
  let kind = if ts.is_missing() {
    format!("MISSING {}", node.kind())
  } else {
    node.kind().to_string()
  };
  PatternTree {
    kind,
    start: ts.start_position().into(),
    end: ts.end_position().into(),
    is_named: node.is_named(),
    children,
    text,
    pattern: None,
  }
}

fn dump_pattern_impl(node: Node<StrDoc<SupportLang>>, pattern: &PatternNode) -> PatternTree {
  use PatternNode as PN;
  let ts = node.get_ts_node();
  let kind = if ts.is_missing() {
    format!("MISSING {}", node.kind())
  } else {
    node.kind().to_string()
  };
  match pattern {
    PN::MetaVar { .. } => {
      let lang = node.lang();
      let expando = lang.expando_char();
      let text = node.text().to_string();
      let text = text.replace(expando, "$");
      PatternTree {
        kind,
        start: ts.start_position().into(),
        end: ts.end_position().into(),
        is_named: true,
        children: vec![],
        text: Some(text),
        pattern: Some(PatternKind::MetaVar),
      }
    }
    PN::Terminal { is_named, .. } => PatternTree {
      kind,
      start: ts.start_position().into(),
      end: ts.end_position().into(),
      is_named: *is_named,
      children: vec![],
      text: Some(node.text().into()),
      pattern: Some(PatternKind::Terminal),
    },
    PN::Internal { children, .. } => {
      let children = children
        .iter()
        .zip(node.children())
        .map(|(pn, n)| dump_pattern_impl(n, pn))
        .collect();
      PatternTree {
        kind,
        start: ts.start_position().into(),
        end: ts.end_position().into(),
        is_named: true,
        children,
        text: None,
        pattern: Some(PatternKind::Internal),
      }
    }
  }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum PatternKind {
  Terminal,
  MetaVar,
  Internal,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternTree {
  kind: String,
  start: Pos,
  end: Pos,
  is_named: bool,
  children: Vec<PatternTree>,
  text: Option<String>,
  pattern: Option<PatternKind>,
}
