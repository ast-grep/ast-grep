import test from 'ava'

import { Lang, parse, parseAsync, type SgNode, type SgRoot } from '../index'
import type TypeScriptTypes from '../lang/TypeScript'

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
  // test parent method
  const parent = child!.parent()
  t.assert(parent!.kind() === root.kind())
  // test find
  const sum = root.find({
    rule: {
      kind: 'binary_expression',
    },
  })!
  t.is(sum.kind(), 'binary_expression')
  t.is(sum.field('operator')!.kind(), '+')

  // test type refinement
  const a = root.find('a')!
  t.assert(a.is('identifier'))
  if (a.is('identifier')) {
    t.assert(a.kind() === 'identifier')
    // @ts-expect-error: should refine kind
    t.assert(a.kind() !== 'invalid')
    t.is(a.field('type_annotation'), null)
  }

  // test rule kind
  t.throws(() => {
    root.find({
      rule: {
        kind: 'notFound', // ok for no type
      },
    })
  })
})

test('test type assertion', t => {
  const sg = parse(Lang.TypeScript, 'a + b') as SgRoot<TypeScriptTypes>
  // test root
  const root = sg.root() as SgNode<TypeScriptTypes, 'program'>
  t.is(root.kind(), 'program')
  // @ts-expect-error
  t.is(root.field('body'), null)
  // test child
  const child = root.child(0) as SgNode<
    TypeScriptTypes,
    'expression_statement' | (string & {})
  >
  t.assert(child !== null)
  const childKind = child!.kind()
  t.assert(childKind === 'expression_statement')
  t.assert(childKind !== ',')
  // test parent method
  const parent = child!.parent()
  t.assert(parent?.kind() === root.kind())
  // test find
  const sum = root.find({
    rule: {
      kind: 'binary_expression',
    },
  }) as SgNode<TypeScriptTypes, 'binary_expression'>
  t.is(sum.kind(), 'binary_expression')
  const kind = sum.field('operator').kind()
  t.assert(kind === '+')
  t.assert(kind !== 'invalid')

  // test type refinement
  const a = root.find('a')!
  t.assert(a.is('identifier'))
  if (a.is('identifier')) {
    t.is(a.kind(), 'identifier')
    // @ts-expect-error: should refine kind
    t.assert(a.kind() !== 'invalid')
    // @ts-expect-error: should reject field
    t.is(a.field('type_annotation'), null)
  }
  // test rule kind
  t.throws(() => {
    root.find({
      rule: {
        // @ts-expect-error: reject kind
        kind: 'notFound',
      },
    })
  })
})

test('test type argument style', t => {
  const sg = parse<TypeScriptTypes>(Lang.TypeScript, 'a + b')
  // test root
  const root = sg.root()
  t.is(root.kind(), 'program')
  // @ts-expect-error
  t.is(root.field('body'), null)
  // test child
  const child = root.child<'expression_statement'>(0)
  t.assert(child !== null)
  const childKind = child!.kind()
  t.is(childKind, 'expression_statement')
  // @ts-expect-error
  t.assert(childKind !== ',')
  // test parent method
  const parent = child!.parent<'program'>()
  t.assert(parent!.kind() === root.kind())
  // @ts-expect-error should reject wrong kind
  const parent2 = child!.parent<'eskdf'>()
  t.assert(parent2!.kind() === root.kind())
  const parent3 = child!.parent<'expression_statement'>()
  // @ts-expect-error parent3 should take new kind
  t.assert(parent3!.kind() === root.kind())
  // @ts-expect-error should report invalid child type
  root.child<'binary_expression'>
  // test find
  const sum = root.find<'binary_expression'>({
    rule: {
      kind: 'binary_expression',
    },
  })!
  t.is(sum.kind(), 'binary_expression')
  const kind = sum.field('operator').kind()
  t.assert(kind === '+')
  t.assert(kind !== 'invalid')

  // test type refinement
  const a = root.find('a')!
  t.assert(a.is('identifier'))
  if (a.is('identifier')) {
    t.assert(a.kind() === 'identifier')
    // @ts-expect-error: should refine kind
    t.assert(a.kind() !== 'invalid')
    // @ts-expect-error: should reject field
    t.is(a.field('type_annotation'), null)
  }
  // okay to use any annotation
  t.is(a.field('type_annotation'), null)

  // test rule kind
  t.throws(() => {
    root.find({
      rule: {
        // @ts-expect-error: reject kind
        kind: 'notFound',
      },
    })
  })
})

test('subtype alias', async t => {
  const sg = await parseAsync<TypeScriptTypes>(
    Lang.TypeScript,
    'export function a() {}',
  )
  const root = sg.root()
  const exp = root.find<'export_statement'>('export function a() {}')!
  const declaration = exp.field('declaration')!
  const kind = declaration.kind()

  // test exhaustive switch
  switch (declaration.kindToRefine) {
    case 'enum_declaration': {
      declaration.field('name')
      // @ts-expect-error: report
      declaration.field('decorator')
      break
    }
    case 'abstract_class_declaration':
    case 'generator_function_declaration':
    case 'ambient_declaration':
    case 'class_declaration':
    case 'function_declaration':
    case 'function_signature':
    case 'import_alias':
    case 'interface_declaration':
    case 'internal_module':
    case 'lexical_declaration':
    case 'module':
    case 'type_alias_declaration':
    case 'variable_declaration':
      break
    default: {
      declaration satisfies never
      t.fail('should not be here')
    }
  }

  // test child kind
  const child = exp.child(0)!
  t.false(child.is('new_expression'))

  t.assert(kind === 'function_declaration')
  t.assert(declaration.kind() !== 'class_declaration')
  // @ts-expect-error: no type alias
  t.assert(declaration.kind() !== 'declaration')
  // @ts-expect-error: kind refined
  t.assert(declaration.kind() !== 'identifier')
  // test rule kind
  const wrong = root.find({
    rule: {
      // @ts-expect-error: reject alias type
      kind: 'declaration',
    },
  })
  t.falsy(wrong)
})
