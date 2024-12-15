import test from 'ava'

import TypeScriptTypes from '../types/TypeScript'
import { Lang, parse, SgNode, SgRoot } from '../index'

test('test no type annotation', t => {
  const sg = parse(Lang.TypeScript, 'a + b')
  // test root kind and field
  const root = sg.root()
  t.is(root.kind(), 'program')
  t.is(root.field('body'), null)
  // test child
  const child = root.child(0)
  t.assert(child !== null)
  const childKind = child!.kind()
  t.assert(childKind === 'expression_statement')
  t.assert(childKind !== ',')
  // test find
  const sum = root.find({
    rule: {
      kind: 'binary_expression',
    },
  })!
  t.is(sum.kind(), 'binary_expression')
  t.is(sum.field('operator')!.kind(), '+')
})


test('test type assertion', t => {
  const sg = parse(Lang.TypeScript, 'a + b: number') as SgRoot<TypeScriptTypes>
  // test root
  const root = sg.root() as SgNode<TypeScriptTypes, 'program'>
  t.is(root.kind(), 'program')
  // @ts-expect-error
  t.is(root.field('body'), null)
  // test child
  const child = root.child(0) as SgNode<TypeScriptTypes, 'expression_statement' | (string & {})>
  t.assert(child !== null)
  const childKind = child!.kind()
  t.assert(childKind === 'expression_statement')
  t.assert(childKind !== ',')
  // test find
  const sum = root.find({
    rule: {
      kind: 'binary_expression',
    },
  }) as SgNode<TypeScriptTypes, 'binary_expression'>
  t.is(sum.kind(), 'binary_expression')
  const kind = sum.field('operator').kind()
  t.assert(kind === '+')
  // @ts-expect-error: we should not report unnamed nodes like +-*/
  t.assert(kind !== 'invalid')
})