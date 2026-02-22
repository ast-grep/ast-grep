use std::rc::Rc;

use crate::ts_types as ts;
use ast_grep_core::matcher::KindMatcher;
use ast_grep_core::source::Content;
use ast_grep_core::{AstGrep, NodeMatch, Pattern};
use wasm_bindgen::prelude::*;

use crate::doc::{WasmConfig, WasmDoc, Wrapper};
#[derive(serde::Serialize, serde::Deserialize)]
#[wasm_bindgen(getter_with_clone)]
pub struct WasmEdit {
  /// The start position of the edit (character offset)
  pub start_pos: u32,
  /// The end position of the edit (character offset)
  pub end_pos: u32,
  /// The text to be inserted
  pub inserted_text: String,
}

#[derive(Clone)]
#[wasm_bindgen(getter_with_clone)]
pub struct Pos {
  /// line number starting from 0
  pub line: u32,
  /// column number starting from 0
  pub column: u32,
  /// character offset of the position
  pub index: u32,
}

#[wasm_bindgen(getter_with_clone)]
pub struct Range {
  /// starting position of the range
  pub start: Pos,
  /// ending position of the range
  pub end: Pos,
}

/// Represents the parsed tree of code.
#[wasm_bindgen]
pub struct SgRoot {
  inner: Rc<AstGrep<WasmDoc>>,
  filename: String,
}

#[wasm_bindgen]
impl SgRoot {
  /// Returns the root SgNode of the ast-grep instance.
  pub fn root(&self) -> SgNode {
    // SAFETY: WasmDoc's Node type wraps a JS SyntaxNode (GC-managed, Clone).
    // It does not actually borrow from the Rust tree. The Rc keeps the
    // AstGrep alive as long as any SgNode references it.
    let root_ref: &'static AstGrep<WasmDoc> =
      unsafe { &*(Rc::as_ptr(&self.inner) as *const AstGrep<WasmDoc>) };
    let node_match: NodeMatch<'static, WasmDoc> = root_ref.root().into();
    SgNode {
      _root: self.inner.clone(),
      inner: node_match,
    }
  }

  /// Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
  /// Returns `"anonymous"` if the instance is created by `parse`.
  pub fn filename(&self) -> String {
    self.filename.clone()
  }

  /// This method is mainly for debugging tree parsing result.
  #[wasm_bindgen(js_name = getInnerTree)]
  pub fn get_inner_tree(&self) -> ts::Tree {
    self.inner.root().get_doc().tree.clone()
  }
}

impl SgRoot {
  pub fn new(inner: AstGrep<WasmDoc>, filename: String) -> Self {
    Self {
      inner: Rc::new(inner),
      filename,
    }
  }
}

/// Represents a single AST node.
#[wasm_bindgen]
pub struct SgNode {
  // Prevent the AstGrep from being dropped while SgNode is alive
  _root: Rc<AstGrep<WasmDoc>>,
  inner: NodeMatch<'static, WasmDoc>,
}

impl SgNode {
  fn make_node(&self, nm: NodeMatch<'static, WasmDoc>) -> SgNode {
    SgNode {
      _root: self._root.clone(),
      inner: nm,
    }
  }

  fn parse_matcher(&self, m: JsValue) -> Result<MatcherType, JsError> {
    if let Some(s) = m.as_string() {
      let lang = *self.inner.lang();
      let pattern = Pattern::try_new(&s, lang).map_err(|e| JsError::new(&e.to_string()))?;
      return Ok(MatcherType::Pattern(pattern));
    }
    if let Some(n) = m.as_f64() {
      return Ok(MatcherType::Kind(KindMatcher::from_id(n as u16)));
    }
    // Treat as WasmConfig object
    let config: WasmConfig = serde_wasm_bindgen::from_value(m)?;
    let lang = *self.inner.lang();
    let rule_core = config.parse_with(lang)?;
    Ok(MatcherType::RuleCore(rule_core))
  }

  // SAFETY helper: transmute NodeMatch lifetime from 'tree to 'static.
  // Safe for WasmDoc because Node wraps a JS GC-managed SyntaxNode.
  unsafe fn cast_match<'t>(nm: NodeMatch<'t, WasmDoc>) -> NodeMatch<'static, WasmDoc> {
    std::mem::transmute(nm)
  }
}

enum MatcherType {
  Pattern(Pattern),
  Kind(KindMatcher),
  RuleCore(ast_grep_config::RuleCore),
}

/// Position and info methods
#[wasm_bindgen]
impl SgNode {
  #[wasm_bindgen(js_name = range)]
  pub fn range(&self) -> Range {
    let byte_range = self.inner.range();
    let start_pos = self.inner.start_pos();
    let end_pos = self.inner.end_pos();
    Range {
      start: Pos {
        line: start_pos.line() as u32,
        column: start_pos.column(self.inner.get_node()) as u32,
        index: byte_range.start as u32,
      },
      end: Pos {
        line: end_pos.line() as u32,
        column: end_pos.column(self.inner.get_node()) as u32,
        index: byte_range.end as u32,
      },
    }
  }

  #[wasm_bindgen(js_name = isLeaf)]
  pub fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }

  #[wasm_bindgen(js_name = isNamed)]
  pub fn is_named(&self) -> bool {
    self.inner.is_named()
  }

  #[wasm_bindgen(js_name = isNamedLeaf)]
  pub fn is_named_leaf(&self) -> bool {
    self.inner.is_named_leaf()
  }

  pub fn kind(&self) -> String {
    self.inner.kind().to_string()
  }

  pub fn is(&self, kind: String) -> bool {
    self.inner.kind() == kind
  }

  pub fn text(&self) -> String {
    self.inner.text().to_string()
  }

  pub fn id(&self) -> u32 {
    self.inner.node_id() as u32
  }
}

/// Matcher methods
#[wasm_bindgen]
impl SgNode {
  pub fn matches(&self, m: JsValue) -> Result<bool, JsError> {
    Ok(match self.parse_matcher(m)? {
      MatcherType::Pattern(p) => self.inner.matches(p),
      MatcherType::Kind(k) => self.inner.matches(k),
      MatcherType::RuleCore(r) => self.inner.matches(r),
    })
  }

  pub fn inside(&self, m: JsValue) -> Result<bool, JsError> {
    Ok(match self.parse_matcher(m)? {
      MatcherType::Pattern(p) => self.inner.inside(p),
      MatcherType::Kind(k) => self.inner.inside(k),
      MatcherType::RuleCore(r) => self.inner.inside(r),
    })
  }

  pub fn has(&self, m: JsValue) -> Result<bool, JsError> {
    Ok(match self.parse_matcher(m)? {
      MatcherType::Pattern(p) => self.inner.has(p),
      MatcherType::Kind(k) => self.inner.has(k),
      MatcherType::RuleCore(r) => self.inner.has(r),
    })
  }

  pub fn precedes(&self, m: JsValue) -> Result<bool, JsError> {
    Ok(match self.parse_matcher(m)? {
      MatcherType::Pattern(p) => self.inner.precedes(p),
      MatcherType::Kind(k) => self.inner.precedes(k),
      MatcherType::RuleCore(r) => self.inner.precedes(r),
    })
  }

  pub fn follows(&self, m: JsValue) -> Result<bool, JsError> {
    Ok(match self.parse_matcher(m)? {
      MatcherType::Pattern(p) => self.inner.follows(p),
      MatcherType::Kind(k) => self.inner.follows(k),
      MatcherType::RuleCore(r) => self.inner.follows(r),
    })
  }

  #[wasm_bindgen(js_name = getMatch)]
  pub fn get_match(&self, m: String) -> Option<SgNode> {
    let node = self.inner.get_env().get_match(&m).cloned()?;
    let nm = NodeMatch::from(node);
    Some(self.make_node(unsafe { Self::cast_match(nm) }))
  }

  #[wasm_bindgen(js_name = getMultipleMatches)]
  pub fn get_multiple_matches(&self, m: String) -> Vec<SgNode> {
    self
      .inner
      .get_env()
      .get_multiple_matches(&m)
      .into_iter()
      .map(|node| {
        let nm = NodeMatch::from(node);
        self.make_node(unsafe { Self::cast_match(nm) })
      })
      .collect()
  }

  #[wasm_bindgen(js_name = getTransformed)]
  pub fn get_transformed(&self, m: String) -> Option<String> {
    let bytes = self.inner.get_env().get_transformed(&m)?;
    Some(Wrapper::encode_bytes(bytes).to_string())
  }
}

/// Tree traversal methods
#[wasm_bindgen]
impl SgNode {
  pub fn children_nodes(&self) -> Vec<SgNode> {
    self
      .inner
      .children()
      .map(|n| {
        let nm = NodeMatch::from(n);
        self.make_node(unsafe { Self::cast_match(nm) })
      })
      .collect()
  }

  pub fn parent_node(&self) -> Option<SgNode> {
    let node = self.inner.parent()?;
    let nm = NodeMatch::from(node);
    Some(self.make_node(unsafe { Self::cast_match(nm) }))
  }

  #[wasm_bindgen(js_name = child)]
  pub fn child_node(&self, nth: u32) -> Option<SgNode> {
    let node = self.inner.child(nth as usize)?;
    let nm = NodeMatch::from(node);
    Some(self.make_node(unsafe { Self::cast_match(nm) }))
  }

  pub fn ancestors(&self) -> Vec<SgNode> {
    self
      .inner
      .ancestors()
      .map(|n| {
        let nm = NodeMatch::from(n);
        self.make_node(unsafe { Self::cast_match(nm) })
      })
      .collect()
  }

  #[wasm_bindgen(js_name = next)]
  pub fn next_node(&self) -> Option<SgNode> {
    let node = self.inner.next()?;
    let nm = NodeMatch::from(node);
    Some(self.make_node(unsafe { Self::cast_match(nm) }))
  }

  #[wasm_bindgen(js_name = nextAll)]
  pub fn next_all(&self) -> Vec<SgNode> {
    self
      .inner
      .next_all()
      .map(|n| {
        let nm = NodeMatch::from(n);
        self.make_node(unsafe { Self::cast_match(nm) })
      })
      .collect()
  }

  #[wasm_bindgen(js_name = prev)]
  pub fn prev_node(&self) -> Option<SgNode> {
    let node = self.inner.prev()?;
    let nm = NodeMatch::from(node);
    Some(self.make_node(unsafe { Self::cast_match(nm) }))
  }

  #[wasm_bindgen(js_name = prevAll)]
  pub fn prev_all(&self) -> Vec<SgNode> {
    self
      .inner
      .prev_all()
      .map(|n| {
        let nm = NodeMatch::from(n);
        self.make_node(unsafe { Self::cast_match(nm) })
      })
      .collect()
  }

  pub fn find(&self, matcher: JsValue) -> Result<Option<SgNode>, JsError> {
    let node_match = match self.parse_matcher(matcher)? {
      MatcherType::Pattern(p) => self.inner.find(p),
      MatcherType::Kind(k) => self.inner.find(k),
      MatcherType::RuleCore(r) => self.inner.find(r),
    };
    Ok(node_match.map(|nm| self.make_node(unsafe { Self::cast_match(nm) })))
  }

  #[wasm_bindgen(js_name = findAll)]
  pub fn find_all(&self, matcher: JsValue) -> Result<Vec<SgNode>, JsError> {
    let matches: Vec<_> = match self.parse_matcher(matcher)? {
      MatcherType::Pattern(p) => self.inner.find_all(p).collect(),
      MatcherType::Kind(k) => self.inner.find_all(k).collect(),
      MatcherType::RuleCore(r) => self.inner.find_all(r).collect(),
    };
    Ok(
      matches
        .into_iter()
        .map(|nm| self.make_node(unsafe { Self::cast_match(nm) }))
        .collect(),
    )
  }

  #[wasm_bindgen(js_name = field)]
  pub fn field_node(&self, name: String) -> Option<SgNode> {
    let node = self.inner.field(&name)?;
    let nm = NodeMatch::from(node);
    Some(self.make_node(unsafe { Self::cast_match(nm) }))
  }

  #[wasm_bindgen(js_name = fieldChildren)]
  pub fn field_children(&self, name: String) -> Vec<SgNode> {
    self
      .inner
      .field_children(&name)
      .map(|n| {
        let nm = NodeMatch::from(n);
        self.make_node(unsafe { Self::cast_match(nm) })
      })
      .collect()
  }
}

/// Edit methods
#[wasm_bindgen]
impl SgNode {
  pub fn replace(&self, text: String) -> WasmEdit {
    let range = self.inner.range();
    WasmEdit {
      start_pos: range.start as u32,
      end_pos: range.end as u32,
      inserted_text: text,
    }
  }

  #[wasm_bindgen(js_name = commitEdits)]
  pub fn commit_edits(&self, edits: JsValue) -> Result<String, JsError> {
    let mut edits: Vec<WasmEdit> = serde_wasm_bindgen::from_value(edits)?;
    edits.sort_by_key(|edit| edit.start_pos);
    let mut new_content = Vec::new();
    let text = self.text();
    let old_content = Wrapper::decode_str(&text);
    let offset = self.inner.range().start;
    let mut start = 0;
    for diff in &edits {
      let pos = diff.start_pos as usize - offset;
      if start > pos {
        continue;
      }
      new_content.extend(&old_content[start..pos]);
      let bytes = Wrapper::decode_str(&diff.inserted_text);
      new_content.extend(&*bytes);
      start = diff.end_pos as usize - offset;
    }
    new_content.extend(&old_content[start..]);
    Ok(Wrapper::encode_bytes(&new_content).to_string())
  }
}
