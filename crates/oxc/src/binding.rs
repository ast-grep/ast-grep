use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::{source::SgNode, Doc, Language};
use oxc_allocator::Allocator;
use oxc_index::Idx;
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};
use std::{borrow::Cow, collections::HashMap, sync::Arc};

#[derive(Clone, Copy)]
pub struct OxcLang(pub SourceType);

const IDS: &[&str] = &[
  "Program",
  "IdentifierName",
  "IdentifierReference",
  "BindingIdentifier",
  "LabelIdentifier",
  "ThisExpression",
  "ArrayExpression",
  "ArrayExpressionElement",
  "Elision",
  "ObjectExpression",
  "ObjectProperty",
  "PropertyKey",
  "TemplateLiteral",
  "TaggedTemplateExpression",
  "MemberExpression",
  "CallExpression",
  "NewExpression",
  "MetaProperty",
  "SpreadElement",
  "Argument",
  "UpdateExpression",
  "UnaryExpression",
  "BinaryExpression",
  "PrivateInExpression",
  "LogicalExpression",
  "ConditionalExpression",
  "AssignmentExpression",
  "AssignmentTarget",
  "SimpleAssignmentTarget",
  "AssignmentTargetPattern",
  "ArrayAssignmentTarget",
  "ObjectAssignmentTarget",
  "AssignmentTargetWithDefault",
  "SequenceExpression",
  "Super",
  "AwaitExpression",
  "ChainExpression",
  "ParenthesizedExpression",
  "Directive",
  "Hashbang",
  "BlockStatement",
  "VariableDeclaration",
  "VariableDeclarator",
  "EmptyStatement",
  "ExpressionStatement",
  "IfStatement",
  "DoWhileStatement",
  "WhileStatement",
  "ForStatement",
  "ForStatementInit",
  "ForInStatement",
  "ForOfStatement",
  "ContinueStatement",
  "BreakStatement",
  "ReturnStatement",
  "WithStatement",
  "SwitchStatement",
  "SwitchCase",
  "LabeledStatement",
  "ThrowStatement",
  "TryStatement",
  "CatchClause",
  "CatchParameter",
  "DebuggerStatement",
  "AssignmentPattern",
  "ObjectPattern",
  "ArrayPattern",
  "BindingRestElement",
  "Function",
  "FormalParameters",
  "FormalParameter",
  "FunctionBody",
  "ArrowFunctionExpression",
  "YieldExpression",
  "Class",
  "ClassBody",
  "MethodDefinition",
  "PropertyDefinition",
  "PrivateIdentifier",
  "StaticBlock",
  "ModuleDeclaration",
  "ImportExpression",
  "ImportDeclaration",
  "ImportSpecifier",
  "ImportDefaultSpecifier",
  "ImportNamespaceSpecifier",
  "ExportNamedDeclaration",
  "ExportDefaultDeclaration",
  "ExportAllDeclaration",
  "ExportSpecifier",
  "V8IntrinsicExpression",
  "BooleanLiteral",
  "NullLiteral",
  "NumericLiteral",
  "StringLiteral",
  "BigIntLiteral",
  "RegExpLiteral",
  "JSXElement",
  "JSXOpeningElement",
  "JSXClosingElement",
  "JSXFragment",
  "JSXElementName",
  "JSXNamespacedName",
  "JSXMemberExpression",
  "JSXMemberExpressionObject",
  "JSXExpressionContainer",
  "JSXAttributeItem",
  "JSXSpreadAttribute",
  "JSXIdentifier",
  "JSXText",
  "TSThisParameter",
  "TSEnumDeclaration",
  "TSEnumBody",
  "TSEnumMember",
  "TSTypeAnnotation",
  "TSLiteralType",
  "TSConditionalType",
  "TSUnionType",
  "TSIntersectionType",
  "TSParenthesizedType",
  "TSIndexedAccessType",
  "TSNamedTupleMember",
  "TSAnyKeyword",
  "TSStringKeyword",
  "TSBooleanKeyword",
  "TSNumberKeyword",
  "TSNeverKeyword",
  "TSIntrinsicKeyword",
  "TSUnknownKeyword",
  "TSNullKeyword",
  "TSUndefinedKeyword",
  "TSVoidKeyword",
  "TSSymbolKeyword",
  "TSThisType",
  "TSObjectKeyword",
  "TSBigIntKeyword",
  "TSTypeReference",
  "TSTypeName",
  "TSQualifiedName",
  "TSTypeParameterInstantiation",
  "TSTypeParameter",
  "TSTypeParameterDeclaration",
  "TSTypeAliasDeclaration",
  "TSClassImplements",
  "TSInterfaceDeclaration",
  "TSPropertySignature",
  "TSMethodSignature",
  "TSConstructSignatureDeclaration",
  "TSInterfaceHeritage",
  "TSModuleDeclaration",
  "TSModuleBlock",
  "TSTypeLiteral",
  "TSInferType",
  "TSTypeQuery",
  "TSImportType",
  "TSMappedType",
  "TSTemplateLiteralType",
  "TSAsExpression",
  "TSSatisfiesExpression",
  "TSTypeAssertion",
  "TSImportEqualsDeclaration",
  "TSModuleReference",
  "TSExternalModuleReference",
  "TSNonNullExpression",
  "Decorator",
  "TSExportAssignment",
  "TSInstantiationExpression",
];

impl Language for OxcLang {
  fn kind_to_id(&self, kind: &str) -> u16 {
    match kind {
      "Program" => 0,
      "IdentifierName" => 1,
      "IdentifierReference" => 2,
      "BindingIdentifier" => 3,
      "LabelIdentifier" => 4,
      "ThisExpression" => 5,
      "ArrayExpression" => 6,
      "ArrayExpressionElement" => 7,
      "Elision" => 8,
      "ObjectExpression" => 9,
      "ObjectProperty" => 10,
      "PropertyKey" => 11,
      "TemplateLiteral" => 12,
      "TaggedTemplateExpression" => 13,
      "MemberExpression" => 14,
      "CallExpression" => 15,
      "NewExpression" => 16,
      "MetaProperty" => 17,
      "SpreadElement" => 18,
      "Argument" => 19,
      "UpdateExpression" => 20,
      "UnaryExpression" => 21,
      "BinaryExpression" => 22,
      "PrivateInExpression" => 23,
      "LogicalExpression" => 24,
      "ConditionalExpression" => 25,
      "AssignmentExpression" => 26,
      "AssignmentTarget" => 27,
      "SimpleAssignmentTarget" => 28,
      "AssignmentTargetPattern" => 29,
      "ArrayAssignmentTarget" => 30,
      "ObjectAssignmentTarget" => 31,
      "AssignmentTargetWithDefault" => 32,
      "SequenceExpression" => 33,
      "Super" => 34,
      "AwaitExpression" => 35,
      "ChainExpression" => 36,
      "ParenthesizedExpression" => 37,
      "Directive" => 38,
      "Hashbang" => 39,
      "BlockStatement" => 40,
      "VariableDeclaration" => 41,
      "VariableDeclarator" => 42,
      "EmptyStatement" => 43,
      "ExpressionStatement" => 44,
      "IfStatement" => 45,
      "DoWhileStatement" => 46,
      "WhileStatement" => 47,
      "ForStatement" => 48,
      "ForStatementInit" => 49,
      "ForInStatement" => 50,
      "ForOfStatement" => 51,
      "ContinueStatement" => 52,
      "BreakStatement" => 53,
      "ReturnStatement" => 54,
      "WithStatement" => 55,
      "SwitchStatement" => 56,
      "SwitchCase" => 57,
      "LabeledStatement" => 58,
      "ThrowStatement" => 59,
      "TryStatement" => 60,
      "CatchClause" => 61,
      "CatchParameter" => 62,
      "DebuggerStatement" => 63,
      "AssignmentPattern" => 64,
      "ObjectPattern" => 65,
      "ArrayPattern" => 66,
      "BindingRestElement" => 67,
      "Function" => 68,
      "FormalParameters" => 69,
      "FormalParameter" => 70,
      "FunctionBody" => 71,
      "ArrowFunctionExpression" => 72,
      "YieldExpression" => 73,
      "Class" => 74,
      "ClassBody" => 75,
      "MethodDefinition" => 76,
      "PropertyDefinition" => 77,
      "PrivateIdentifier" => 78,
      "StaticBlock" => 79,
      "ModuleDeclaration" => 80,
      "ImportExpression" => 81,
      "ImportDeclaration" => 82,
      "ImportSpecifier" => 83,
      "ImportDefaultSpecifier" => 84,
      "ImportNamespaceSpecifier" => 85,
      "ExportNamedDeclaration" => 86,
      "ExportDefaultDeclaration" => 87,
      "ExportAllDeclaration" => 88,
      "ExportSpecifier" => 89,
      "V8IntrinsicExpression" => 90,
      "BooleanLiteral" => 91,
      "NullLiteral" => 92,
      "NumericLiteral" => 93,
      "StringLiteral" => 94,
      "BigIntLiteral" => 95,
      "RegExpLiteral" => 96,
      "JSXElement" => 97,
      "JSXOpeningElement" => 98,
      "JSXClosingElement" => 99,
      "JSXFragment" => 100,
      "JSXElementName" => 101,
      "JSXNamespacedName" => 102,
      "JSXMemberExpression" => 103,
      "JSXMemberExpressionObject" => 104,
      "JSXExpressionContainer" => 105,
      "JSXAttributeItem" => 106,
      "JSXSpreadAttribute" => 107,
      "JSXIdentifier" => 108,
      "JSXText" => 109,
      "TSThisParameter" => 110,
      "TSEnumDeclaration" => 111,
      "TSEnumBody" => 112,
      "TSEnumMember" => 113,
      "TSTypeAnnotation" => 114,
      "TSLiteralType" => 115,
      "TSConditionalType" => 116,
      "TSUnionType" => 117,
      "TSIntersectionType" => 118,
      "TSParenthesizedType" => 119,
      "TSIndexedAccessType" => 120,
      "TSNamedTupleMember" => 121,
      "TSAnyKeyword" => 122,
      "TSStringKeyword" => 123,
      "TSBooleanKeyword" => 124,
      "TSNumberKeyword" => 125,
      "TSNeverKeyword" => 126,
      "TSIntrinsicKeyword" => 127,
      "TSUnknownKeyword" => 128,
      "TSNullKeyword" => 129,
      "TSUndefinedKeyword" => 130,
      "TSVoidKeyword" => 131,
      "TSSymbolKeyword" => 132,
      "TSThisType" => 133,
      "TSObjectKeyword" => 134,
      "TSBigIntKeyword" => 135,
      "TSTypeReference" => 136,
      "TSTypeName" => 137,
      "TSQualifiedName" => 138,
      "TSTypeParameterInstantiation" => 139,
      "TSTypeParameter" => 140,
      "TSTypeParameterDeclaration" => 141,
      "TSTypeAliasDeclaration" => 142,
      "TSClassImplements" => 143,
      "TSInterfaceDeclaration" => 144,
      "TSPropertySignature" => 145,
      "TSMethodSignature" => 146,
      "TSConstructSignatureDeclaration" => 147,
      "TSInterfaceHeritage" => 148,
      "TSModuleDeclaration" => 149,
      "TSModuleBlock" => 150,
      "TSTypeLiteral" => 151,
      "TSInferType" => 152,
      "TSTypeQuery" => 153,
      "TSImportType" => 154,
      "TSMappedType" => 155,
      "TSTemplateLiteralType" => 156,
      "TSAsExpression" => 157,
      "TSSatisfiesExpression" => 158,
      "TSTypeAssertion" => 159,
      "TSImportEqualsDeclaration" => 160,
      "TSModuleReference" => 161,
      "TSExternalModuleReference" => 162,
      "TSNonNullExpression" => 163,
      "Decorator" => 164,
      "TSExportAssignment" => 165,
      "TSInstantiationExpression" => 166,
      _ => panic!("Unknown kind: {kind}"),
    }
  }
  fn field_to_id(&self, _field: &str) -> Option<u16> {
    None
  }
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|s| OxcDoc::try_new(s.to_string(), *self))
  }
}

#[derive(Clone, Debug)]
struct InnerOxc {
  id: NodeId,
  kind_id: u16,
  parent_id: Option<NodeId>,
  span: oxc_span::Span,
  children: Vec<NodeId>,
}

fn convert(nodes: &AstNodes) -> HashMap<NodeId, InnerOxc> {
  let mut pool = HashMap::default();
  for node in nodes.iter() {
    let id = node.id();
    let span = node.span();
    let parent_id = nodes.parent_id(node.id());
    let kind_id = node.kind().ty() as u16;
    pool.insert(
      id,
      InnerOxc {
        id,
        kind_id,
        parent_id,
        span,
        children: vec![],
      },
    );
    if let Some(parent_id) = parent_id {
      if let Some(parent) = pool.get_mut(&parent_id) {
        parent.children.push(id);
      }
    }
  }
  pool
}

#[derive(Clone)]
pub struct OxcNode<'a> {
  inner: InnerOxc,
  pool: &'a HashMap<NodeId, InnerOxc>,
}

impl<'a> SgNode<'a> for OxcNode<'a> {
  fn parent(&self) -> Option<Self> {
    let parent = self.pool.get(self.inner.parent_id.as_ref()?)?;
    Some(OxcNode {
      inner: parent.clone(),
      pool: self.pool,
    })
  }
  fn children(&self) -> impl ExactSizeIterator<Item = Self> {
    let ret: Vec<_> = self
      .inner
      .children
      .iter()
      .filter_map(|child_id| {
        self.pool.get(child_id).map(|child| OxcNode {
          inner: child.clone(),
          pool: self.pool,
        })
      })
      .collect();
    ret.into_iter()
  }
  fn kind(&self) -> Cow<str> {
    IDS[self.inner.kind_id as usize].into()
  }
  fn node_id(&self) -> usize {
    self.inner.id.index()
  }
  fn kind_id(&self) -> u16 {
    self.inner.kind_id
  }
  fn range(&self) -> std::ops::Range<usize> {
    self.inner.span.start as usize..self.inner.span.end as usize
  }
  fn start_pos(&self) -> ast_grep_core::Position {
    todo!("not implemented")
  }
  fn end_pos(&self) -> ast_grep_core::Position {
    todo!("not implemented")
  }
}

#[derive(Clone)]
pub struct OxcDoc {
  _allocator: Arc<Allocator>,
  source: String,
  lang: OxcLang,
  pool: HashMap<NodeId, InnerOxc>,
  root_id: Option<NodeId>,
}

fn parse(
  allocator: &Allocator,
  source_text: &String,
  lang: OxcLang,
) -> Result<(HashMap<NodeId, InnerOxc>, Option<NodeId>), String> {
  // Parse the source text into an AST
  let parser_ret = Parser::new(allocator, source_text, lang.0).parse();
  if !parser_ret.errors.is_empty() {
    let error_message: String = parser_ret
      .errors
      .into_iter()
      .map(|error| {
        format!(
          "{:?}\n",
          error.with_source_code(Arc::new(source_text.to_string()))
        )
      })
      .collect();
    return Err(error_message);
  }
  let program = parser_ret.program;

  let semantic = SemanticBuilder::new()
    // Enable additional syntax checks not performed by the parser
    .with_check_syntax_error(true)
    .build(&program);

  if !semantic.errors.is_empty() {
    let error_message: String = semantic
      .errors
      .into_iter()
      .map(|error| {
        format!(
          "{:?}\n",
          error.with_source_code(Arc::new(source_text.to_string()))
        )
      })
      .collect();
    return Err(error_message);
  }
  let nodes = semantic.semantic.nodes();
  let pool = convert(nodes);
  let root_id = nodes.root();
  Ok((pool, root_id))
}

impl OxcDoc {
  pub fn try_new(source: String, lang: OxcLang) -> Result<Self, String> {
    let allocator = Allocator::default();
    let (pool, root_id) = parse(&allocator, &source, lang)?;
    Ok(Self {
      _allocator: Arc::new(allocator),
      source,
      lang,
      pool,
      root_id,
    })
  }
}

impl Doc for OxcDoc {
  type Source = String;
  type Lang = OxcLang;
  type Node<'r> = OxcNode<'r>;

  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Source {
    &self.source
  }
  fn do_edit(&mut self, _edit: &ast_grep_core::source::Edit<Self::Source>) -> Result<(), String> {
    todo!("not implemented")
  }
  fn root_node(&self) -> Self::Node<'_> {
    let root_id = self.root_id.expect("Root node not found");
    let inner = self.pool.get(&root_id).expect("Root node not found");
    OxcNode {
      inner: inner.clone(),
      pool: &self.pool,
    }
  }
  fn get_node_text<'a>(&'a self, node: &Self::Node<'a>) -> Cow<'a, str> {
    Cow::Borrowed(&self.source[node.range()])
  }
}
