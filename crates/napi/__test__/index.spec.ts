import test from 'ava'

import { js, parseFiles } from '../index'
const { parse, kind } = js

test('find from native code', t => {
  const sg = parse('console.log(123)')
  const match = sg.root().find('console.log')
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
  const node = match!.find('console')
  t.deepEqual(node!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 7, index: 7 },
  })
})

test('findAll from native code', t => {
  const sg = parse('console.log(123); let a = console.log.bind(console);')
  const match = sg.root().findAll('console.log')
  t.deepEqual(match.length, 2)
  t.deepEqual(match[0].range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
  t.deepEqual(match[1].range(), {
    start: { line: 0, column: 26, index: 26 },
    end: { line: 0, column: 37, index: 37 },
  })
})

test('find not match', t => {
  const sg = parse('console.log(123)')
  const match = sg.root().find('notExist')
  t.is(match, null)
})

test('get variable', t => {
  const sg = parse('console.log("hello world")')
  const match = sg.root().find('console.log($MATCH)')
  t.is(match!.getMatch('MATCH')!.text(), '"hello world"')
})

test('find by kind', t => {
  const sg = parse('console.log("hello world")')
  const match = sg.root().find(kind('member_expression'))
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
})

test('find by config', t => {
  const sg = parse('console.log("hello world")')
  const match = sg.root().find({
    rule: {kind: 'member_expression'},
  })
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
})

test('test find files', async t => {
  await parseFiles(['./__test__/index.spec.ts'], (err, tree) => {
    t.is(err, null)
    t.is(tree.filename(), './__test__/index.spec.ts')
    t.assert(tree.root() !== null)
  })
})

test('test file count', async t => {
  let i = 0
  let fileCount: number | undefined = undefined
  fileCount = await parseFiles(['./'], (err, _) => {
    // ZZZ... sleep a while to mock expensive operation
    let start = Date.now()
    while (Date.now() - start < 1) continue
    t.is(err, null)
    i += 1;
    if (fileCount) console.log(_.filename())
  })
  t.is(i, fileCount)
})
