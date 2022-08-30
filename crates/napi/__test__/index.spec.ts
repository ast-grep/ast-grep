import test from 'ava'

import { AstGrep } from '../index'

test('findByString from native code', t => {
  const sg = AstGrep.js('console.log(123)')
  const match = sg.findByString('console')
  t.deepEqual(match.range(), {
    start: { row: 0, col: 0 },
    end: { row: 0, col: 7 },
  })
})
