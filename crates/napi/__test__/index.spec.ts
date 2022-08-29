import test from 'ava'

import { findNodes } from '../index'

test('sync function from native code', (t) => {
  t.deepEqual(findNodes('console.log(123)', 'console'), [
    {
      start: {
        row: 0,
        col: 0,
      },
      end: {
        row: 0,
        col: 7,
      },
    },
  ])
})
