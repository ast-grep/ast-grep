use core::cell::RefCell;
use js_sys::{Array, Error, JsString, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

pub trait JsValueExt {
  type Value;
  fn lift_error(self) -> Result<Self::Value, JsError>;
}

impl<T> JsValueExt for Result<T, JsValue> {
  type Value = T;

  fn lift_error(self) -> Result<Self::Value, JsError> {
    self.map_err(|err| {
      let message = match err.dyn_into::<Error>() {
        Ok(error) => error.message(),
        Err(value) => JsString::from(value),
      };
      JsError::new(&String::from(message))
    })
  }
}

thread_local! {
    // Ensure `web-tree-sitter` is only initialized once
    static TREE_SITTER_INITIALIZED: RefCell<bool> = const { RefCell::new(false) };
}

pub struct TreeSitter;

impl TreeSitter {
  pub async fn init() -> Result<(), JsError> {
    #![allow(non_snake_case)]

    // Exit early if `web-tree-sitter` is already initialized
    if TREE_SITTER_INITIALIZED.with(|cell| *cell.borrow()) {
      return Ok(());
    }

    JsFuture::from(Parser::init()).await.lift_error()?;

    // Set `web-tree-sitter` to initialized
    TREE_SITTER_INITIALIZED.with(|cell| cell.replace(true));

    Ok(())
  }

  pub fn init_guard() {
    if !TREE_SITTER_INITIALIZED.with(|cell| *cell.borrow()) {
      wasm_bindgen::throw_str("TreeSitter::init must be called to initialize the library");
    }
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug, Eq, PartialEq)]
  #[wasm_bindgen(extends = Object)]
  pub type Edit;

  // Instance Properties

  #[wasm_bindgen(method, getter, js_name = newEndIndex)]
  pub fn new_end_index(this: &Edit) -> u32;

  #[wasm_bindgen(method, getter, js_name = newEndPosition)]
  pub fn new_end_position(this: &Edit) -> Point;

  #[wasm_bindgen(method, getter, js_name = oldEndIndex)]
  pub fn old_end_index(this: &Edit) -> u32;

  #[wasm_bindgen(method, getter, js_name = oldEndPosition)]
  pub fn old_end_position(this: &Edit) -> Point;

  #[wasm_bindgen(method, getter, js_name = startIndex)]
  pub fn start_index(this: &Edit) -> u32;

  #[wasm_bindgen(method, getter, js_name = startPosition)]
  pub fn start_position(this: &Edit) -> Point;
}

impl Edit {
  pub fn new(
    start_index: u32,
    old_end_index: u32,
    new_end_index: u32,
    start_position: &Point,
    old_end_position: &Point,
    new_end_position: &Point,
  ) -> Self {
    let obj = Object::new();
    Reflect::set(&obj, &"startIndex".into(), &start_index.into()).unwrap();
    Reflect::set(&obj, &"oldEndIndex".into(), &old_end_index.into()).unwrap();
    Reflect::set(&obj, &"newEndIndex".into(), &new_end_index.into()).unwrap();
    Reflect::set(&obj, &"startPosition".into(), &start_position.into()).unwrap();
    Reflect::set(&obj, &"oldEndPosition".into(), &old_end_position.into()).unwrap();
    Reflect::set(&obj, &"newEndPosition".into(), &new_end_position.into()).unwrap();
    JsCast::unchecked_into(obj)
  }
}

impl Default for Edit {
  fn default() -> Self {
    let start_index = Default::default();
    let old_end_index = Default::default();
    let new_end_index = Default::default();
    let start_position = &Default::default();
    let old_end_position = &Default::default();
    let new_end_position = &Default::default();
    Self::new(
      start_index,
      old_end_index,
      new_end_index,
      start_position,
      old_end_position,
      new_end_position,
    )
  }
}

#[wasm_bindgen(module = "web-tree-sitter")]
extern "C" {
  #[derive(Clone, Debug, PartialEq)]
  pub type Language;

  // Static Methods

  #[wasm_bindgen(static_method_of = Language, js_name = load)]
  fn __load_bytes(bytes: &Uint8Array) -> Promise;

  #[wasm_bindgen(static_method_of = Language, js_name = load)]
  fn __load_path(path: &str) -> Promise;

  // Instance Properties

  #[wasm_bindgen(method, getter, js_name = abiVersion)]
  pub fn abi_version(this: &Language) -> u32;

  #[wasm_bindgen(method, getter, js_name = fieldCount)]
  pub fn field_count(this: &Language) -> u16;

  #[wasm_bindgen(method, getter, js_name = nodeTypeCount)]
  pub fn node_kind_count(this: &Language) -> u16;

  // Instance Methods

  #[wasm_bindgen(method, js_name = fieldNameForId)]
  pub fn field_name_for_id(this: &Language, field_id: u16) -> Option<String>;

  #[wasm_bindgen(method, js_name = fieldIdForName)]
  pub fn field_id_for_name(this: &Language, field_name: &str) -> Option<u16>;

  #[wasm_bindgen(method, js_name = idForNodeType)]
  pub fn id_for_node_kind(this: &Language, kind: &str, named: bool) -> u16;

  #[wasm_bindgen(method, js_name = nodeTypeForId)]
  pub fn node_kind_for_id(this: &Language, kind_id: u16) -> Option<String>;

  #[wasm_bindgen(method, js_name = nodeTypeIsNamed)]
  pub fn node_kind_is_named(this: &Language, kind_id: u16) -> bool;

  #[wasm_bindgen(method, js_name = nodeTypeIsVisible)]
  pub fn node_kind_is_visible(this: &Language, kind_id: u16) -> bool;
}

impl Language {
  pub async fn load_bytes(bytes: &Uint8Array) -> Result<Language, LanguageError> {
    TreeSitter::init_guard();
    JsFuture::from(Language::__load_bytes(bytes))
      .await
      .map(JsCast::unchecked_into)
      .map_err(JsCast::unchecked_into)
  }

  pub async fn load_path(path: &str) -> Result<Language, LanguageError> {
    TreeSitter::init_guard();
    JsFuture::from(Language::__load_path(path))
      .await
      .map(JsCast::unchecked_into)
      .map_err(JsCast::unchecked_into)
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug, Eq, PartialEq)]
  #[wasm_bindgen(extends = Error)]
  pub type LanguageError;
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug, Eq, PartialEq)]
  #[wasm_bindgen(extends = Object)]
  pub type ParseOptions;

  // Instance Properties

  // -> Range[]
  #[wasm_bindgen(method, getter, js_name = includedRanges)]
  pub fn included_ranges(this: &ParseOptions) -> Option<Array>;
}

impl ParseOptions {
  pub fn new(included_ranges: Option<&Array>) -> Self {
    let obj = Object::new();
    Reflect::set(&obj, &"includedRanges".into(), &included_ranges.into()).unwrap();
    JsCast::unchecked_into(obj)
  }
}

impl Default for ParseOptions {
  fn default() -> Self {
    let included_ranges = Default::default();
    Self::new(included_ranges)
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug)]
  #[wasm_bindgen(extends = Object)]
  pub type Point;

  // Instance Properties

  #[wasm_bindgen(method, getter)]
  pub fn column(this: &Point) -> u32;

  #[wasm_bindgen(method, getter)]
  pub fn row(this: &Point) -> u32;
}

impl Point {
  pub fn new(row: u32, column: u32) -> Self {
    let obj = Object::new();
    Reflect::set(&obj, &"row".into(), &row.into()).unwrap();
    Reflect::set(&obj, &"column".into(), &column.into()).unwrap();
    JsCast::unchecked_into(obj)
  }

  #[inline(always)]
  fn spread(&self) -> (u32, u32) {
    (self.row(), self.column())
  }
}

impl Default for Point {
  fn default() -> Self {
    let row = Default::default();
    let column = Default::default();
    Self::new(row, column)
  }
}

impl Eq for Point {}

impl std::hash::Hash for Point {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    let this = self.spread();
    this.hash(state);
  }
}

impl Ord for Point {
  fn cmp(&self, that: &Self) -> std::cmp::Ordering {
    let this = self.spread();
    let that = that.spread();
    this.cmp(&that)
  }
}

impl PartialEq for Point {
  fn eq(&self, that: &Self) -> bool {
    let this = self.spread();
    let that = that.spread();
    this.eq(&that)
  }
}

impl PartialOrd for Point {
  fn partial_cmp(&self, that: &Point) -> Option<std::cmp::Ordering> {
    Some(self.cmp(that))
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug)]
  pub type SyntaxNode;

  // Instance Properties

  #[wasm_bindgen(method, getter, js_name = childCount)]
  pub fn child_count(this: &SyntaxNode) -> u32;

  #[wasm_bindgen(method, getter)]
  pub fn children(this: &SyntaxNode) -> Box<[JsValue]>;

  #[wasm_bindgen(method, getter, js_name = endIndex)]
  pub fn end_index(this: &SyntaxNode) -> u32;

  #[wasm_bindgen(method, getter, js_name = endPosition)]
  pub fn end_position(this: &SyntaxNode) -> Point;

  #[wasm_bindgen(method, getter, js_name = firstChild)]
  pub fn first_child(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = firstNamedChild)]
  pub fn first_named_child(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter)]
  pub fn id(this: &SyntaxNode) -> u32;

  #[wasm_bindgen(method, getter, js_name = lastChild)]
  pub fn last_child(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = lastNamedChild)]
  pub fn last_named_child(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = namedChildCount)]
  pub fn named_child_count(this: &SyntaxNode) -> u32;

  #[wasm_bindgen(method, getter, js_name = namedChildren)]
  pub fn named_children(this: &SyntaxNode) -> Box<[JsValue]>;

  #[wasm_bindgen(method, getter, js_name = nextNamedSibling)]
  pub fn next_named_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = nextSibling)]
  pub fn next_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter)]
  pub fn parent(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = previousNamedSibling)]
  pub fn previous_named_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = previousSibling)]
  pub fn previous_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, getter, js_name = startIndex)]
  pub fn start_index(this: &SyntaxNode) -> u32;

  #[wasm_bindgen(method, getter, js_name = startPosition)]
  pub fn start_position(this: &SyntaxNode) -> Point;

  #[wasm_bindgen(method, getter)]
  pub fn text(this: &SyntaxNode) -> JsString;

  #[wasm_bindgen(method, getter)]
  pub fn tree(this: &SyntaxNode) -> Tree;

  #[wasm_bindgen(method, getter, js_name = type)]
  pub fn type_(this: &SyntaxNode) -> JsString;

  #[wasm_bindgen(method, getter, js_name = typeId)]
  pub fn type_id(this: &SyntaxNode) -> u16;

  // reference: https://github.com/tree-sitter/tree-sitter/pull/3103/files
  #[wasm_bindgen(method, getter, js_name = isNamed)]
  pub fn is_named(this: &SyntaxNode) -> bool;

  #[wasm_bindgen(method, getter, js_name = isMissing)]
  pub fn is_missing(this: &SyntaxNode) -> bool;

  #[wasm_bindgen(method, getter, js_name = hasChanges)]
  pub fn has_changes(this: &SyntaxNode) -> bool;

  #[wasm_bindgen(method, getter, js_name = hasError)]
  pub fn has_error(this: &SyntaxNode) -> bool;

  #[wasm_bindgen(method, getter, js_name = isError)]
  pub fn is_error(this: &SyntaxNode) -> bool;

  // Instance Methods

  #[wasm_bindgen(method)]
  pub fn child(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = childForFieldId)]
  pub fn child_for_field_id(this: &SyntaxNode, field_id: u16) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = childForFieldName)]
  pub fn child_for_field_name(this: &SyntaxNode, field_name: &str) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = childWithDescendant)]
  pub fn child_with_descendant(this: &SyntaxNode, descendant: &SyntaxNode) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = descendantForIndex)]
  pub fn descendant_for_index(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = descendantForIndex)]
  pub fn descendant_for_index_range(
    this: &SyntaxNode,
    start_index: u32,
    end_index: u32,
  ) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = descendantForPosition)]
  pub fn descendant_for_position(this: &SyntaxNode, position: &Point) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = descendantForPosition)]
  pub fn descendant_for_position_range(
    this: &SyntaxNode,
    start_position: &Point,
    end_position: &Point,
  ) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = descendantsOfType)]
  pub fn descendants_of_type_array(
    this: &SyntaxNode,
    type_: Box<[JsValue]>,
    start_position: Option<&Point>,
    end_position: Option<&Point>,
  ) -> Box<[JsValue]>;

  // -> SyntaxNode[]
  #[wasm_bindgen(method, js_name = descendantsOfType)]
  pub fn descendants_of_type_string(
    this: &SyntaxNode,
    type_: &str,
    start_position: Option<&Point>,
    end_position: Option<&Point>,
  ) -> Box<[JsValue]>;

  #[wasm_bindgen(method)]
  pub fn equals(this: &SyntaxNode, other: &SyntaxNode) -> bool;

  #[wasm_bindgen(method, js_name = namedChild)]
  pub fn named_child(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = namedDescendantForIndex)]
  pub fn named_descendant_for_index(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = namedDescendantForIndex)]
  pub fn named_descendant_for_index_range(
    this: &SyntaxNode,
    start_index: u32,
    end_index: u32,
  ) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = namedDescendantForPosition)]
  pub fn named_descendant_for_position(this: &SyntaxNode, position: &Point) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = namedDescendantForPosition)]
  pub fn named_descendant_for_position_range(
    this: &SyntaxNode,
    start_position: &Point,
    end_position: &Point,
  ) -> Option<SyntaxNode>;

  #[wasm_bindgen(method, js_name = toString)]
  pub fn to_string(this: &SyntaxNode) -> JsString;

  #[wasm_bindgen(method)]
  pub fn walk(this: &SyntaxNode) -> TreeCursor;
}

impl PartialEq<SyntaxNode> for SyntaxNode {
  fn eq(&self, other: &SyntaxNode) -> bool {
    self.equals(other)
  }
}

impl Eq for SyntaxNode {}

impl std::hash::Hash for SyntaxNode {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.id().hash(state);
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Debug)]
  pub type Tree;

  // Instance Properties

  #[wasm_bindgen(method, getter, js_name = rootNode)]
  pub fn root_node(this: &Tree) -> SyntaxNode;
  #[wasm_bindgen(method, getter)]
  pub fn language(this: &Tree) -> Language;

  // Instance Methods

  #[wasm_bindgen(method)]
  pub fn copy(this: &Tree) -> Tree;

  #[wasm_bindgen(method)]
  pub fn delete(this: &Tree);

  #[wasm_bindgen(method)]
  pub fn edit(this: &Tree, delta: &Edit) -> Tree;

  #[wasm_bindgen(method)]
  pub fn walk(this: &Tree) -> TreeCursor;

  // -> Range[]
  #[wasm_bindgen(method, js_name = getChangedRanges)]
  pub fn get_changed_ranges(this: &Tree, other: &Tree) -> Box<[JsValue]>;
}

impl Clone for Tree {
  fn clone(&self) -> Tree {
    self.copy()
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug)]
  pub type TreeCursor;

  // Instance Properties

  #[wasm_bindgen(method, getter, js_name = endIndex)]
  pub fn end_index(this: &TreeCursor) -> u32;

  #[wasm_bindgen(method, getter, js_name = endPosition)]
  pub fn end_position(this: &TreeCursor) -> Point;

  #[wasm_bindgen(method, getter, js_name = nodeIsNamed)]
  pub fn node_is_named(this: &TreeCursor) -> bool;

  #[wasm_bindgen(method, getter, js_name = nodeText)]
  pub fn node_text(this: &TreeCursor) -> JsString;

  #[wasm_bindgen(method, getter, js_name = nodeType)]
  pub fn node_type(this: &TreeCursor) -> JsString;

  #[wasm_bindgen(method, getter, js_name = startIndex)]
  pub fn start_index(this: &TreeCursor) -> u32;

  #[wasm_bindgen(method, getter, js_name = startPosition)]
  pub fn start_position(this: &TreeCursor) -> Point;

  #[wasm_bindgen(method, getter, js_name = currentFieldId)]
  pub fn current_field_id(this: &TreeCursor) -> Option<u16>;

  #[wasm_bindgen(method, getter, js_name = currentFieldName)]
  pub fn current_field_name(this: &TreeCursor) -> Option<JsString>;

  #[wasm_bindgen(method, getter, js_name = currentNode)]
  pub fn current_node(this: &TreeCursor) -> SyntaxNode;

  // Instance Methods

  #[wasm_bindgen(method)]
  pub fn delete(this: &TreeCursor);

  #[wasm_bindgen(method, js_name = gotoFirstChild)]
  pub fn goto_first_child(this: &TreeCursor) -> bool;

  #[wasm_bindgen(method, js_name = gotoNextSibling)]
  pub fn goto_next_sibling(this: &TreeCursor) -> bool;

  #[wasm_bindgen(method, js_name = gotoPreviousSibling)]
  pub fn goto_previous_sibling(this: &TreeCursor) -> bool;

  #[wasm_bindgen(method, js_name = gotoFirstChildForIndex)]
  pub fn goto_first_child_for_index(this: &TreeCursor, index: u32) -> bool;

  #[wasm_bindgen(method, js_name = gotoParent)]
  pub fn goto_parent(this: &TreeCursor) -> bool;

  #[wasm_bindgen(method)]
  pub fn reset(this: &TreeCursor, node: &SyntaxNode);
}

#[wasm_bindgen(module = "web-tree-sitter")]
extern "C" {
  #[derive(Clone, Debug)]
  pub type Parser;

  // Static Methods
  #[wasm_bindgen(static_method_of = Parser)]
  pub fn init() -> Promise;

  // Constructor

  #[wasm_bindgen(catch, constructor)]
  fn __new() -> Result<Parser, ParserError>;

  // Instance Properties

  #[wasm_bindgen(method, getter)]
  pub fn language(this: &Parser) -> Option<Language>;

  // Instance Methods

  #[wasm_bindgen(method)]
  pub fn delete(this: &Parser);

  #[wasm_bindgen(catch, method, js_name = parse)]
  pub fn parse_with_string(
    this: &Parser,
    input: &JsString,
    previous_tree: Option<&Tree>,
    options: Option<&ParseOptions>,
  ) -> Result<Option<Tree>, ParserError>;

  #[wasm_bindgen(method)]
  pub fn reset(this: &Parser);

  #[wasm_bindgen(catch, method, js_name = setLanguage)]
  pub fn set_language(this: &Parser, language: Option<&Language>) -> Result<(), LanguageError>;
}

impl Parser {
  pub fn new() -> Result<Parser, ParserError> {
    TreeSitter::init_guard();
    let result = Parser::__new()?;
    Ok(result)
  }
}

#[wasm_bindgen]
extern "C" {
  #[derive(Clone, Debug, Eq, PartialEq)]
  #[wasm_bindgen(extends = Error)]
  pub type ParserError;
}
