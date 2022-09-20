import test from 'ava'

import { AstGrep } from '../index'

test('findByString from native code', t => {
  const sg = AstGrep.js('console.log(123)')
  const match = sg.root().findByString('console.log')
  t.deepEqual(match!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 11, index: 11 },
  })
  const node = match.findByString('console')
  t.deepEqual(node!.range(), {
    start: { line: 0, column: 0, index: 0 },
    end: { line: 0, column: 7, index: 7 },
  })
})

test('findAll from native code', t => {
  const sg = AstGrep.js('console.log(123); let a = console.log.bind(console);')
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

test('findByString not match', t => {
  const sg = AstGrep.js('console.log(123)')
  const match = sg.root().findByString('notExist')
  t.is(match, null)
})
