import b from 'benny'
import fs from 'fs'

import { ts as sg } from '@ast-grep/napi'
import * as babel from '@babel/parser'
import oxc from '@oxidation-compiler/napi'
import * as swc from '@swc/core'
import * as ts from 'typescript'
import Parser from 'tree-sitter'
// because tree-sitter-typescript does not have d.ts
const tresSitterTS = require('tree-sitter-typescript').typescript

async function run() {
  const treeSitter = new Parser()
  treeSitter.setLanguage(tresSitterTS)
  const tsEntry = fs.readFileSync('./fixtures/tsc.ts.fixture', 'utf8')
  const vueRef = fs.readFileSync('./fixtures/ref.ts.fixture', 'utf8')
  const tsChecker = fs.readFileSync('./fixtures/checker.ts.fixture', 'utf8')
  const cases = [
    ['Parse One Line', 'let a = 123'],
    ['Parse Small File', tsEntry],
    ['Parse Medium File', vueRef],
    ['Parse Huge File', tsChecker],
  ]
  for (const [title, source] of cases) {
    await b.suite(
      title,
      b.add('ast-grep parse', () => {
        sg.parse(source)
      }),
      b.add('tree-sitter parse', () => {
        treeSitter.parse(source)
      }),
      b.add('babel parse', () => {
        babel.parse(source, {
          plugins: ['typescript'],
          sourceType: 'module',
        })
      }),
      b.add('oxc parse(false positive for now)', () => {
        oxc.parseSync(source, {
          sourceType: 'module'
        })
      }),
      b.add('swc parse', () => {
        swc.parseSync(source, {
          syntax: 'typescript',
        })
      }),
      b.add('TypeScript parse', () => {
        ts.createSourceFile(
          'benchmark.ts',
          source,
          ts.ScriptTarget.Latest,
        )
      }),
      b.cycle(),
      b.complete(),
    )
  }
}

run().catch((e) => {
  console.error(e)
})
