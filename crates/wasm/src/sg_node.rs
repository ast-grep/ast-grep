use ast_grep_config::RuleCore;
use ast_grep_core::{matcher::KindMatcher, AstGrep, NodeMatch, Pattern, StrDoc};
use ast_grep_language::SupportLang;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::config::parse_config_from_js_value;

pub enum JsMatcher {
  Pattern(String),
  Kind(u16),
  Config(RuleCore<SupportLang>),
}

// We may not need these anymore as we're using the types.d.ts file
// But keeping them for clarity in the Rust code
#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(typescript_type = "NodeRange")]
  pub type INodeRange;

  #[wasm_bindgen(typescript_type = "Matcher")]
  pub type IMatcher;

  #[wasm_bindgen(typescript_type = "Edit")]
  pub type IEdit;

  #[wasm_bindgen(typescript_type = "Edit[]")]
  pub type IEditArray;
}

#[derive(Serialize, Deserialize)]
struct Position {
  row: usize,
  column: usize,
  index: usize,
}

#[derive(Serialize, Deserialize)]
struct NodeRange {
  start: Position,
  end: Position,
}

#[derive(Serialize, Deserialize)]
pub struct Edit {
  /// The start position of the edit
  pub start_pos: u32,
  /// The end position of the edit
  pub end_pos: u32,
  /// The text to be inserted
  pub inserted_text: String,
}

// Node struct that holds a reference to the root
#[wasm_bindgen(skip_typescript)]
pub struct SgNode {
  // Use Rc to reference count the root, ensuring it stays alive as long as any nodes exist
  pub(crate) root: Rc<SgRoot>,
  pub(crate) inner: NodeMatch<'static, StrDoc<SupportLang>>,
}

#[wasm_bindgen]
impl SgNode {
  #[wasm_bindgen]
  pub fn text(&self) -> String {
    self.inner.text().to_string()
  }

  /// Check if the node is the same kind as the given `kind` string
  #[wasm_bindgen]
  pub fn is(&self, kind: String) -> bool {
    self.inner.kind() == kind
  }

  #[wasm_bindgen]
  pub fn kind(&self) -> String {
    self.inner.kind().to_string()
  }

  #[wasm_bindgen]
  pub fn range(&self) -> INodeRange {
    let start_pos = self.inner.start_pos();
    let end_pos = self.inner.end_pos();
    let byte_range = self.inner.range();

    let result = NodeRange {
      start: Position {
        row: start_pos.line(),
        column: start_pos.column(&self.inner),
        index: byte_range.start,
      },
      end: Position {
        row: end_pos.line(),
        column: end_pos.column(&self.inner),
        index: byte_range.end,
      },
    };

    serde_wasm_bindgen::to_value(&result).unwrap().into()
  }

  /// Check if the node is a leaf node (has no children)
  #[wasm_bindgen(js_name = isLeaf)]
  pub fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }

  /// Check if the node is a named node
  #[wasm_bindgen(js_name = isNamed)]
  pub fn is_named(&self) -> bool {
    self.inner.is_named()
  }

  /// Check if the node is a named leaf node
  #[wasm_bindgen(js_name = isNamedLeaf)]
  pub fn is_named_leaf(&self) -> bool {
    self.inner.is_named_leaf()
  }

  /// Find a node matching the given pattern, kind, or config
  #[wasm_bindgen]
  pub fn find(&self, value: IMatcher) -> Option<SgNode> {
    let lang = *self.inner.lang();
    let matcher = convert_js_matcher(value.into(), lang);

    match matcher {
      JsMatcher::Pattern(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        self.inner.find(pattern).map(|node| SgNode {
          root: self.root.clone(),
          inner: node,
        })
      }
      JsMatcher::Kind(kind) => {
        let matcher = KindMatcher::from_id(kind);
        self.inner.find(matcher).map(|node| SgNode {
          root: self.root.clone(),
          inner: node,
        })
      }
      JsMatcher::Config(config) => self.inner.find(config).map(|node| SgNode {
        root: self.root.clone(),
        inner: node,
      }),
    }
  }

  /// Find all nodes matching the given pattern, kind, or config
  #[wasm_bindgen(js_name = findAll)]
  pub fn find_all(&self, value: IMatcher) -> Vec<SgNode> {
    let lang = *self.inner.lang();
    let matcher = convert_js_matcher(value.into(), lang);

    match matcher {
      JsMatcher::Pattern(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        self
          .inner
          .find_all(pattern)
          .map(|node| SgNode {
            root: self.root.clone(),
            inner: node,
          })
          .collect()
      }
      JsMatcher::Kind(kind) => {
        let matcher = KindMatcher::from_id(kind);
        self
          .inner
          .find_all(matcher)
          .map(|node| SgNode {
            root: self.root.clone(),
            inner: node,
          })
          .collect()
      }
      JsMatcher::Config(config) => self
        .inner
        .find_all(config)
        .map(|node| SgNode {
          root: self.root.clone(),
          inner: node,
        })
        .collect(),
    }
  }

  /// Check if the node matches the given pattern, kind, or config
  #[wasm_bindgen]
  pub fn matches(&self, value: IMatcher) -> bool {
    let lang = *self.inner.lang();
    let matcher = convert_js_matcher(value.into(), lang);

    match matcher {
      JsMatcher::Pattern(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        self.inner.matches(pattern)
      }
      JsMatcher::Kind(kind) => {
        let matcher = KindMatcher::from_id(kind);
        self.inner.matches(matcher)
      }
      JsMatcher::Config(config) => self.inner.matches(config),
    }
  }

  /// Check if the node is inside a node matching the given pattern, kind, or config
  #[wasm_bindgen]
  pub fn inside(&self, value: IMatcher) -> bool {
    let lang = *self.inner.lang();
    let matcher = convert_js_matcher(value.into(), lang);

    match matcher {
      JsMatcher::Pattern(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        self.inner.inside(pattern)
      }
      JsMatcher::Kind(kind) => {
        let matcher = KindMatcher::from_id(kind);
        self.inner.inside(matcher)
      }
      JsMatcher::Config(config) => self.inner.inside(config),
    }
  }

  /// Check if the node has a child matching the given pattern, kind, or config
  #[wasm_bindgen]
  pub fn has(&self, value: IMatcher) -> bool {
    let lang = *self.inner.lang();
    let matcher = convert_js_matcher(value.into(), lang);

    match matcher {
      JsMatcher::Pattern(pattern) => {
        let pattern = Pattern::new(&pattern, lang);
        self.inner.has(pattern)
      }
      JsMatcher::Kind(kind) => {
        let matcher = KindMatcher::from_id(kind);
        self.inner.has(matcher)
      }
      JsMatcher::Config(config) => self.inner.has(config),
    }
  }

  /// Get the parent node
  #[wasm_bindgen]
  pub fn parent(&self) -> Option<SgNode> {
    self.inner.parent().map(|node| SgNode {
      root: self.root.clone(),
      inner: node.into(),
    })
  }

  /// Get a child node at the given index
  #[wasm_bindgen]
  pub fn child(&self, nth: usize) -> Option<SgNode> {
    self.inner.child(nth).map(|node| SgNode {
      root: self.root.clone(),
      inner: node.into(),
    })
  }

  /// Get all child nodes
  #[wasm_bindgen]
  pub fn children(&self) -> Vec<SgNode> {
    self
      .inner
      .children()
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node.into(),
      })
      .collect()
  }

  /// Get all ancestor nodes
  #[wasm_bindgen]
  pub fn ancestors(&self) -> Vec<SgNode> {
    self
      .inner
      .ancestors()
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node.into(),
      })
      .collect()
  }

  /// Get the next sibling node
  #[wasm_bindgen]
  pub fn next(&self) -> Option<SgNode> {
    self.inner.next().map(|node| SgNode {
      root: self.root.clone(),
      inner: node.into(),
    })
  }

  /// Get all next sibling nodes
  #[wasm_bindgen(js_name = nextAll)]
  pub fn next_all(&self) -> Vec<SgNode> {
    self
      .inner
      .next_all()
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node.into(),
      })
      .collect()
  }

  /// Get the previous sibling node
  #[wasm_bindgen]
  pub fn prev(&self) -> Option<SgNode> {
    self.inner.prev().map(|node| SgNode {
      root: self.root.clone(),
      inner: node.into(),
    })
  }

  /// Get all previous sibling nodes
  #[wasm_bindgen(js_name = prevAll)]
  pub fn prev_all(&self) -> Vec<SgNode> {
    self
      .inner
      .prev_all()
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node.into(),
      })
      .collect()
  }

  /// Get a field node by name
  #[wasm_bindgen]
  pub fn field(&self, name: String) -> Option<SgNode> {
    self.inner.field(&name).map(|node| SgNode {
      root: self.root.clone(),
      inner: node.into(),
    })
  }

  /// Get all field children by name
  #[wasm_bindgen(js_name = fieldChildren)]
  pub fn field_children(&self, name: String) -> Vec<SgNode> {
    self
      .inner
      .field_children(&name)
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node.into(),
      })
      .collect()
  }

  #[wasm_bindgen(js_name = getMatch)]
  pub fn get_match(&self, m: String) -> Result<Option<SgNode>, JsError> {
    let node = self
      .inner
      .get_env()
      .get_match(&m)
      .cloned()
      .map(NodeMatch::from)
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node,
      });

    Ok(node)
  }

  #[wasm_bindgen(js_name = getMultipleMatches)]
  pub fn get_multiple_matches(&self, m: String) -> Result<Vec<SgNode>, JsError> {
    let nodes = self
      .inner
      .get_env()
      .get_multiple_matches(&m)
      .into_iter()
      .map(NodeMatch::from)
      .map(|node| SgNode {
        root: self.root.clone(),
        inner: node,
      })
      .collect();

    Ok(nodes)
  }

  #[wasm_bindgen(js_name = "commitEdits")]
  pub fn commit_edits(&self, edits: IEditArray) -> Result<String, JsError> {
    let mut edits: Vec<Edit> = serde_wasm_bindgen::from_value(edits.into())?;
    edits.sort_by_key(|edit| edit.start_pos);
    let mut new_content = String::new();
    let old_content = self.text();

    let offset = self.inner.range().start;
    let mut start = 0;
    for diff in edits {
      let pos = diff.start_pos as usize - offset;
      // skip overlapping edits
      if start > pos {
        continue;
      }
      new_content.push_str(&old_content[start..pos]);
      new_content.push_str(&diff.inserted_text);
      start = diff.end_pos as usize - offset;
    }
    // add trailing statements
    new_content.push_str(&old_content[start..]);
    Ok(new_content)
  }

  #[wasm_bindgen]
  pub fn replace(&self, text: String) -> IEdit {
    let byte_range = self.inner.range();
    serde_wasm_bindgen::to_value(&Edit {
      start_pos: byte_range.start as u32,
      end_pos: byte_range.end as u32,
      inserted_text: text,
    })
    .unwrap()
    .into()
  }
}

// Wrapper for AstGrep to expose to JavaScript
#[wasm_bindgen(skip_typescript)]
pub struct SgRoot {
  pub(crate) inner: AstGrep<StrDoc<SupportLang>>,
}

#[wasm_bindgen]
impl SgRoot {
  #[wasm_bindgen]
  pub fn root(&self) -> Result<SgNode, JsError> {
    // Create a new SgRoot with the same source and language
    let root = Rc::new(SgRoot {
      inner: AstGrep::new(self.inner.source(), *self.inner.lang()),
    });

    // Create a NodeMatch from the root node
    // We need to use a safe approach to handle the lifetime
    let node = {
      let node = root.inner.root();
      // Convert to NodeMatch which is safer to work with
      NodeMatch::from(node)
    };

    // Use a type with 'static lifetime but ensure it's tied to the root's lifetime
    // through the Rc reference counting
    let node: NodeMatch<'static, _> = unsafe {
      // This is still unsafe but not as bad because:
      // 1. We're using Rc to ensure the root stays alive
      // 2. NodeMatch is designed to work with this pattern
      std::mem::transmute(node)
    };

    Ok(SgNode { root, inner: node })
  }

  #[wasm_bindgen]
  pub fn source(&self) -> String {
    self.inner.source().to_string()
  }
}

fn convert_js_matcher(matcher: JsValue, lang: SupportLang) -> JsMatcher {
  if matcher.is_string() {
    JsMatcher::Pattern(matcher.as_string().unwrap())
  } else if matcher.is_object() {
    JsMatcher::Config(parse_config_from_js_value(lang, matcher).unwrap())
  } else {
    JsMatcher::Kind(matcher.as_f64().unwrap() as u16)
  }
}
