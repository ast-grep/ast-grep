import test from 'ava'
import { createRequire } from 'module'

const require = createRequire(import.meta.url)
const wasm = require('../pkg')
const { parse, kind } = wasm

test.before(async () => {
  await wasm.initializeTreeSitter()
  const parserPath = require.resolve(
    'tree-sitter-javascript/tree-sitter-javascript.wasm',
  )
  await wasm.registerDynamicLanguage({
    javascript: { libraryPath: parserPath },
  })
})

test('find from wasm', t => {
  const sg = parse('javascript', 'console.log(123)')
  const match = sg.root().find('console.log')
  t.truthy(match)
  const range = match.range()
  t.is(range.start.line, 0)
  t.is(range.start.column, 0)
  t.is(range.start.index, 0)
  t.is(range.end.line, 0)
  t.is(range.end.column, 11)
  t.is(range.end.index, 11)
})

test('find nested node', t => {
  const sg = parse('javascript', 'console.log(123)')
  const match = sg.root().find('console.log')
  const node = match.find('console')
  t.truthy(node)
  const range = node.range()
  t.is(range.start.index, 0)
  t.is(range.end.index, 7)
})

test('find multiple nodes', t => {
  const sg = parse('javascript', 'a(1, 2, 3)')
  const match = sg.root().find('a($$$B)')
  t.truthy(match)
  const matchedVar = match.getMultipleMatches('B')
  const start = matchedVar[0].range().start
  const end = matchedVar[matchedVar.length - 1].range().end
  t.is(start.index, 2)
  t.is(end.index, 9)
})

test('find unicode', t => {
  const str = `console.log("Hello, 世界")
  print("ザ・ワールド")`
  const sg = parse('javascript', str)
  const match = sg.root().find('console.log($_)')
  t.truthy(match)
  t.is(match.range().start.line, 0)
  t.is(match.range().start.column, 0)
  const node = sg.root().find('print($_)')
  t.truthy(node)
  t.is(node.range().start.line, 1)
  t.is(node.range().start.column, 2)
})

test('find with transformation', t => {
  const sg = parse('javascript', 'console.log("Hello, 世界")')
  const match = sg.root().find({
    rule: { pattern: 'console.log($A)' },
    transform: {
      NEW_ARG: {
        substring: { source: '$A', startChar: 1, endChar: -1 },
      },
    },
  })
  t.truthy(match)
  t.is(match.getTransformed('NEW_ARG'), 'Hello, 世界')
  t.is(match.getMatch('A').text(), '"Hello, 世界"')
})

test('code fix', t => {
  const sg = parse('javascript', 'a = console.log(123)')
  const match = sg.root().find('console.log')
  const fix = match.replace('console.error')
  t.is(fix.inserted_text, 'console.error')
  t.is(fix.start_pos, 4)
  t.is(fix.end_pos, 15)
  t.is(match.commitEdits([fix]), 'console.error')
  const newCode = sg.root().commitEdits([fix])
  t.is(newCode, 'a = console.error(123)')
})

test('multiple fixes with unicode', t => {
  const sg = parse('javascript', 'いいよ = log(123) + log(456)')
  const matches = sg.root().findAll(kind('javascript', 'number'))
  const fixes = matches.map(m => m.replace('114514'))
  fixes.sort((a, b) => b.start_pos - a.start_pos)
  const newCode = sg.root().commitEdits(fixes)
  t.is(newCode, 'いいよ = log(114514) + log(114514)')
})

test('fix with user defined range', t => {
  const sg = parse('javascript', 'いいよ = log(123)')
  const match = sg.root().find(kind('javascript', 'number'))
  const edit = match.replace('514')
  edit.start_pos -= 1
  edit.end_pos += 1
  const newCode = sg.root().commitEdits([edit])
  t.is(newCode, 'いいよ = log514')
})

test('findAll', t => {
  const sg = parse(
    'javascript',
    'console.log(123); let a = console.log.bind(console);',
  )
  const matches = sg.root().findAll('console.log')
  t.is(matches.length, 2)
  t.is(matches[0].range().start.index, 0)
  t.is(matches[0].range().end.index, 11)
  t.is(matches[1].range().start.index, 26)
  t.is(matches[1].range().end.index, 37)
})

test('find not match', t => {
  const sg = parse('javascript', 'console.log(123)')
  const match = sg.root().find('notExist')
  t.is(match, undefined)
})

test('get variable', t => {
  const sg = parse('javascript', 'console.log("hello world")')
  const match = sg.root().find('console.log($MATCH)')
  t.is(match.getMatch('MATCH').text(), '"hello world"')
})

test('find by kind', t => {
  const sg = parse('javascript', 'console.log("hello world")')
  const match = sg.root().find(kind('javascript', 'member_expression'))
  t.truthy(match)
  t.is(match.range().start.index, 0)
  t.is(match.range().end.index, 11)
})

test('find by config', t => {
  const sg = parse('javascript', 'console.log("hello world")')
  const match = sg.root().find({ rule: { kind: 'member_expression' } })
  t.truthy(match)
  t.is(match.range().start.index, 0)
  t.is(match.range().end.index, 11)
})

test('node matches pattern', t => {
  const sg = parse('javascript', 'console.log(123)')
  const match = sg.root().find({ rule: { kind: 'call_expression' } })
  t.true(match.matches('console.log($$$)'))
  t.false(match.matches('console.log'))
})

test('node matches config', t => {
  const sg = parse('javascript', 'console.log(123)')
  const match = sg.root().find('console.log($$$)')
  t.true(match.matches({ rule: { kind: 'call_expression' } }))
  t.false(match.matches({ rule: { kind: 'identifier' } }))
})

test('node follows', t => {
  const sg = parse('javascript', 'const a = 1; const b = 2;')
  const a = sg.root().find('const a = 1')
  const b = sg.root().find('const b = 2')
  t.false(a.follows('const b = 2'))
  t.true(b.follows('const a = 1'))
})

test('node precedes', t => {
  const sg = parse('javascript', 'const a = 1; const b = 2;')
  const a = sg.root().find('const a = 1')
  const b = sg.root().find('const b = 2')
  t.true(a.precedes('const b = 2'))
  t.false(b.precedes('const a = 1'))
})

test('node inside', t => {
  const sg = parse('javascript', 'if (true) { const x = 1; }')
  const match = sg.root().find('const x = 1')
  t.true(match.inside('if (true) { $$$ }'))
  t.false(match.inside('function() { $$$ }'))
})

test('node has', t => {
  const sg = parse('javascript', 'if (true) { const x = 1; }')
  const match = sg.root().find('if (true) { $$$ }')
  t.true(match.has('const x = 1'))
  t.false(match.has('const y = 2'))
})

test('node id consistency', t => {
  const sg = parse('javascript', 'console.log(123)')
  const byPattern = sg.root().find('console.log($$$)')
  const byKind = sg.root().find(kind('javascript', 'call_expression'))
  t.is(byPattern.id(), byKind.id())
})

test('node properties', t => {
  const sg = parse('javascript', 'console.log(123)')
  const root = sg.root()
  t.false(root.isLeaf())
  t.true(root.isNamed())
  const num = root.find(kind('javascript', 'number'))
  t.true(num.isNamedLeaf())
  t.true(num.isNamed())
  t.is(num.text(), '123')
  t.true(num.is('number'))
  t.false(num.is('string'))
})

test('tree traversal', t => {
  const sg = parse('javascript', 'a; b; c;')
  const root = sg.root()
  const children = root.children_nodes()
  const named = children.filter(c => c.isNamed())
  t.true(named.length >= 3)
  const parent = named[0].parent_node()
  t.truthy(parent)
  t.is(parent.kind(), 'program')
})

test('next and prev', t => {
  const sg = parse('javascript', 'const a = 1; const b = 2;')
  const a = sg.root().find('const a = 1')
  t.truthy(a.next())
  const b = sg.root().find('const b = 2')
  t.truthy(b.prev())
})

test('ancestors', t => {
  const sg = parse('javascript', 'if (true) { const x = 1; }')
  const x = sg.root().find('const x = 1')
  const ancestors = x.ancestors()
  t.true(ancestors.length >= 2)
  const kinds = ancestors.map(a => a.kind())
  t.true(kinds.includes('program'))
})

test('field access', t => {
  const sg = parse('javascript', 'function foo(a, b) { return a; }')
  const func = sg.root().find(kind('javascript', 'function_declaration'))
  const name = func.field('name')
  t.truthy(name)
  t.is(name.text(), 'foo')
})

test('filename', t => {
  const sg = parse('javascript', 'const a = 1')
  t.is(sg.filename(), 'anonymous')
})

test('kind function', t => {
  const k = kind('javascript', 'identifier')
  t.true(k > 0)
})

test('pattern function', t => {
  const result = wasm.pattern('javascript', 'console.log($A)')
  t.truthy(result)
})

// --- dumpPattern ---

test('dumpPattern simple metavar', t => {
  // '$VAR' is 4 chars, JS expando is '$' so no preprocessing
  const dump = wasm.dumpPattern('javascript', '$VAR')
  t.is(dump.pattern, 'metaVar')
  t.is(dump.text, '$VAR')
  t.is(dump.isNamed, true)
  t.is(dump.children.length, 0)
  t.is(dump.start.line, 0)
  t.is(dump.start.column, 0)
  t.is(dump.end.line, 0)
  t.is(dump.end.column, 4)
})

test('dumpPattern with nested nodes', t => {
  // 'console.log($MSG)' = 17 chars; '(' is at col 11, '$MSG' at 12–16
  const dump = wasm.dumpPattern('javascript', 'console.log($MSG)')
  t.is(dump.kind, 'call_expression')
  t.is(dump.pattern, 'internal')
  t.is(dump.start.line, 0)
  t.is(dump.start.column, 0)
  t.is(dump.end.line, 0)
  t.is(dump.end.column, 17)
  // should have children: member_expression and arguments
  t.true(dump.children.length >= 2)
  const args = dump.children.find(c => c.kind === 'arguments')
  t.truthy(args)
  const metaVar = args.children.find(c => c.pattern === 'metaVar')
  t.truthy(metaVar)
  t.is(metaVar.text, '$MSG')
  t.is(metaVar.start.line, 0)
  t.is(metaVar.start.column, 12)
  t.is(metaVar.end.column, 16)
})

test('dumpPattern with let declaration', t => {
  // 'let $A = $B': $A at col 4–6, $B at col 9–11, total 11 chars
  const dump = wasm.dumpPattern('javascript', 'let $A = $B')
  t.is(dump.kind, 'lexical_declaration')
  t.is(dump.start.line, 0)
  t.is(dump.start.column, 0)
  t.is(dump.end.line, 0)
  t.is(dump.end.column, 11)
  const declarator = dump.children.find(c => c.kind === 'variable_declarator')
  t.truthy(declarator)
  const metaVars = declarator.children.filter(c => c.pattern === 'metaVar')
  t.is(metaVars.length, 2)
  t.is(metaVars[0].text, '$A')
  t.is(metaVars[0].start.column, 4)
  t.is(metaVars[0].end.column, 6)
  t.is(metaVars[1].text, '$B')
  t.is(metaVars[1].start.column, 9)
  t.is(metaVars[1].end.column, 11)
})

test('dumpPattern with selector', t => {
  // 'class A { $F = $I }': field_definition starts at col 10 ($F), ends at col 17 ($I end)
  // $F at col 10–12, $I at col 15–17
  const dump = wasm.dumpPattern(
    'javascript',
    'class A { $F = $I }',
    'field_definition',
  )
  t.is(dump.kind, 'field_definition')
  t.is(dump.pattern, 'internal')
  t.is(dump.start.line, 0)
  t.is(dump.start.column, 10)
  t.is(dump.end.line, 0)
  t.is(dump.end.column, 17)
  const metaVars = dump.children.filter(c => c.pattern === 'metaVar')
  t.is(metaVars.length, 2)
  t.is(metaVars[0].start.column, 10)
  t.is(metaVars[0].end.column, 12)
  t.is(metaVars[1].start.column, 15)
  t.is(metaVars[1].end.column, 17)
})

test('dumpPattern with strictness', t => {
  // 'let $A = $B' = 11 chars; strictness only affects matching, not position dump
  const smart = wasm.dumpPattern('javascript', 'let $A = $B')
  t.is(smart.kind, 'lexical_declaration')
  t.is(smart.start.line, 0)
  t.is(smart.start.column, 0)
  t.is(smart.end.column, 11)

  const ast = wasm.dumpPattern('javascript', 'let $A = $B', null, 'ast')
  t.is(ast.kind, 'lexical_declaration')
  t.is(ast.start.line, 0)
  t.is(ast.start.column, 0)
  t.is(ast.end.column, 11)
})

test('dumpPattern invalid pattern', t => {
  t.throws(() => wasm.dumpPattern('javascript', ''))
})

test('dumpPattern invalid strictness', t => {
  t.throws(() => wasm.dumpPattern('javascript', '$A', null, 'invalid'))
})

test('invalid language', t => {
  t.throws(() => parse('not_a_language', 'code'))
})

test('invalid config error', t => {
  const sg = parse('javascript', 'console.log(123)')
  t.throws(() => sg.root().find({ rule: { regex: '(' } }))
})

// --- Multi-language support ---

test('parse multiple languages simultaneously', async t => {
  const pythonPath = require.resolve(
    'tree-sitter-python/tree-sitter-python.wasm',
  )
  await wasm.registerDynamicLanguage({
    python: { libraryPath: pythonPath, expandoChar: 'µ' },
  })

  // Parse JavaScript (still works)
  const jsSg = parse('javascript', 'console.log(123)')
  const jsMatch = jsSg.root().find('console.log')
  t.truthy(jsMatch)
  t.is(jsMatch.range().start.index, 0)
  t.is(jsMatch.range().end.index, 11)

  // Parse Python
  const pySg = parse('python', "print('hello')")
  t.is(pySg.root().kind(), 'module')
  const pyMatch = pySg.root().find("print('hello')")
  t.truthy(pyMatch)

  // JavaScript still works after Python
  const jsSg2 = parse('javascript', 'let x = 1')
  const jsMatch2 = jsSg2.root().find('let x = 1')
  t.truthy(jsMatch2)
})

test('kind works for multiple languages', async t => {
  const pythonPath = require.resolve(
    'tree-sitter-python/tree-sitter-python.wasm',
  )
  await wasm.registerDynamicLanguage({
    python: { libraryPath: pythonPath, expandoChar: 'µ' },
  })

  const jsKind = kind('javascript', 'identifier')
  const pyKind = kind('python', 'identifier')
  t.true(jsKind > 0)
  t.true(pyKind > 0)
})

// --- getInnerTree ---

test('getInnerTree returns a tree-sitter Tree', t => {
  const sg = parse('javascript', 'console.log(123)')
  const tree = sg.getInnerTree()
  t.truthy(tree)
})

test('getInnerTree rootNode is program', t => {
  const sg = parse('javascript', 'console.log(123)')
  const tree = sg.getInnerTree()
  t.is(tree.rootNode.type, 'program')
})

test('getInnerTree rootNode has children', t => {
  const sg = parse('javascript', 'a; b; c;')
  const tree = sg.getInnerTree()
  t.true(tree.rootNode.childCount >= 3)
})

test('getInnerTree walk traverses tree', t => {
  const sg = parse('javascript', 'let x = 1')
  const tree = sg.getInnerTree()
  const cursor = tree.walk()
  t.is(cursor.nodeType, 'program')
  t.true(cursor.gotoFirstChild())
  t.is(cursor.nodeType, 'lexical_declaration')
})
