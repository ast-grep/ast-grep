import b from 'benny'
import fs from 'fs'

import { ts as sg } from '@ast-grep/napi'
import * as babel from '@babel/core'
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

export function parseSyncBench(source: string) {
  return {
    'ast-grep sync parse': () => {
      sg.parse(source)
    },
    'tree-sitter sync parse': () => {
      treeSitter.parse(source)
    },
    'babel sync parse': () => {
      babel.parseSync(source, {
        plugins: ['@babel/plugin-syntax-typescript'],
        sourceType: 'module',
      })
    },
    'oxc sync parse': () => {
      JSON.parse(
        oxc.parseSync(source, {
          sourceType: 'module',
          sourceFilename: 'test.ts',
        }).program,
      )
    },
    'swc sync parse': () => {
      swc.parseSync(source, {
        syntax: 'typescript',
      })
    },
    'TypeScript sync parse': () => {
      ts.createSourceFile('benchmark.ts', source, ts.ScriptTarget.Latest)
    },
  }
}

function parseAsyncBench(source: string) {
  return {
    'ast-grep async parse': () => sg.parseAsync(source),
    'tree-sitter parse(not async)': () => {
      treeSitter.parse(source)
    },
    'babel async parse': () =>
      babel.parseAsync(source, {
        plugins: ['@babel/plugin-syntax-typescript'],
        sourceType: 'module',
      }),
    'oxc async parse': async () => {
      const src = await oxc.parseAsync(source, {
        sourceType: 'module',
        sourceFilename: 'test.ts',
      })
      JSON.parse(src.program)
    },
    'swc async parse': () =>
      swc.parse(source, {
        syntax: 'typescript',
      }),
    'TypeScript parse(not async)': () => {
      ts.createSourceFile('benchmark.ts', source, ts.ScriptTarget.Latest)
    },
  }
}

async function run(func: (s: string) => Record<string, () => unknown>) {
  const cases = prepareCases()
  for (const [title, source] of cases) {
    const benches = func(source)
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

run(parseAsyncBench).catch((e) => {
  console.error(e)
})

run(parseSyncBench).catch((e) => {
  console.error(e)
})
