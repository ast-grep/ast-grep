import test from 'ava'

import { AstGrep } from '../index'

test('findByString from native code', t => {
  const sg = AstGrep.js('console.log(123)')
  const match = sg.root().findByString('console.log')
  t.deepEqual(match.range(), {
    start: { row: 0, col: 0 },
    end: { row: 0, col: 11 },
  })
  const node = match.findByString('console')
  t.deepEqual(node.range(), {
    start: { row: 0, col: 0 },
    end: { row: 0, col: 7 },
  })
})

test('findAll from native code', t => {
  const sg = AstGrep.js('console.log(123); let a = console.log.bind(console);')
  const match = sg.root().findAll('console.log')
  t.deepEqual(match.length, 2)
  t.deepEqual(match[0].range(), {
    start: { row: 0, col: 0 },
    end: { row: 0, col: 11 },
  })
  t.deepEqual(match[1].range(), {
    start: { row: 0, col: 26 },
    end: { row: 0, col: 37 },
  })
})
