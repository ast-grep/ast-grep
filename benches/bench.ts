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

const treeSitter = new Parser()
treeSitter.setLanguage(tresSitterTS)

function prepareCases() {
  const tsEntry = fs.readFileSync('./fixtures/tsc.ts.fixture', 'utf8')
  const vueRef = fs.readFileSync('./fixtures/ref.ts.fixture', 'utf8')
  const tsChecker = fs.readFileSync('./fixtures/checker.ts.fixture', 'utf8')
  return [
    ['Parse One Line', 'let a = 123'],
    ['Parse Small File', tsEntry],
    ['Parse Medium File', vueRef],
    ['Parse Huge File', tsChecker],
  ]
}

function parseSyncBench(source: string) {
  return {
    'ast-grep parse': () => {
      sg.parse(source)
    },
    'tree-sitter parse': () => {
      treeSitter.parse(source)
    },
    'babel parse': () => {
      babel.parse(source, {
        plugins: ['typescript'],
        sourceType: 'module',
      })
    },
    'oxc parse': () => {
      oxc.parseSync(source, {
        sourceType: 'module',
        sourceFilename: 'test.ts',
      })
    },
    'swc parse': () => {
      swc.parseSync(source, {
        syntax: 'typescript',
      })
    },
    'TypeScript parse': () => {
      ts.createSourceFile('benchmark.ts', source, ts.ScriptTarget.Latest)
    },
  }
}

async function run() {
  const cases = prepareCases()
  for (const [title, source] of cases) {
    const benches = parseSyncBench(source)
    await b.suite(
      title,
      ...Object.entries(benches).map(([runnerName, runner]) =>
        b.add(runnerName, runner),
      ),
      b.cycle(),
      b.complete(),
    )
  }
}

run().catch((e) => {
  console.error(e)
})
