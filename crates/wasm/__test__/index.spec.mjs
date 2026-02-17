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
  t.truthy(a.next_node())
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
  const dump = wasm.dumpPattern('javascript', '$VAR')
  t.is(dump.isMetaVar, true)
  t.is(dump.kind, 'MetaVar')
  t.is(dump.text, '$VAR')
  t.is(dump.children.length, 0)
})

test('dumpPattern with nested nodes', t => {
  const dump = wasm.dumpPattern('javascript', 'console.log($MSG)')
  t.is(dump.isMetaVar, false)
  t.is(dump.kind, 'call_expression')
  // should have children: member_expression and arguments
  t.true(dump.children.length >= 2)
  // find the MetaVar in the tree
  const args = dump.children.find(c => c.kind === 'arguments')
  t.truthy(args)
  const metaVar = args.children.find(c => c.isMetaVar)
  t.truthy(metaVar)
  t.is(metaVar.text, '$MSG')
})

test('dumpPattern with let declaration', t => {
  const dump = wasm.dumpPattern('javascript', 'let $A = $B')
  t.is(dump.kind, 'lexical_declaration')
  // find variable_declarator child
  const declarator = dump.children.find(c => c.kind === 'variable_declarator')
  t.truthy(declarator)
  const metaVars = declarator.children.filter(c => c.isMetaVar)
  t.is(metaVars.length, 2)
  t.is(metaVars[0].text, '$A')
  t.is(metaVars[1].text, '$B')
})

test('dumpPattern with selector', t => {
  const dump = wasm.dumpPattern(
    'javascript',
    'class A { $F = $I }',
    'field_definition',
  )
  t.is(dump.kind, 'field_definition')
  const metaVars = dump.children.filter(c => c.isMetaVar)
  t.is(metaVars.length, 2)
})

test('dumpPattern with strictness ast', t => {
  // With "smart" (default), unnamed tokens like "let" and "=" are included
  const smart = wasm.dumpPattern('javascript', 'let $A = $B')
  const smartTexts = smart.children
    .find(c => c.kind === 'variable_declarator')
    .children.map(c => c.text || c.kind)
  t.true(smartTexts.includes('='))

  // With "ast" strictness, unnamed tokens are excluded
  const ast = wasm.dumpPattern('javascript', 'let $A = $B', null, 'ast')
  const astTexts = ast.children
    .find(c => c.kind === 'variable_declarator')
    .children.map(c => c.text || c.kind)
  t.false(astTexts.includes('='))
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
