import test from 'ava'

import { findInFiles, parse, registerDynamicLanguage } from '../index'

const { platform, arch } = process

const isAppleSilicon = platform === 'darwin' && arch === 'arm64'
const isX64Linux = platform === 'linux' && arch === 'x64'
const canTestDynamicLang = isAppleSilicon || isX64Linux

if (isAppleSilicon) {
  registerDynamicLanguage({
    json: {
      libraryPath: '../../fixtures/json-mac.so',
      languageSymbol: 'tree_sitter_json',
      extensions: ['json'],
    },
  })
} else if (isX64Linux) {
  registerDynamicLanguage({
    json: {
      libraryPath: '../../fixtures/json-linux.so',
      languageSymbol: 'tree_sitter_json',
      extensions: ['json'],
    },
  })
}

test('test load custom lang', t => {
  if (!canTestDynamicLang) {
    t.pass('This test is not available on this platform')
    return
  }
  const sg = parse('json', '{"test": 123}')
  const root = sg.root()
  const node = root.find('123')!
  t.truthy(node)
  t.is(node.kind(), 'number')
  const no = root.find('456')
  t.falsy(no)
})

test('discover file', async t => {
  if (!canTestDynamicLang) {
    t.pass('This test is not available on this platform')
    return
  }
  await findInFiles('json', {
    paths: ['../'],
    matcher: {
      rule: {
        kind: 'string',
      },
    },
  }, (error, nodes) => {
    t.falsy(error)
    t.truthy(nodes)
    t.is(nodes[0].kind(), 'string')
    const file = nodes[0].getRoot().filename()
    t.assert(file.endsWith('.json'))
  })
})
