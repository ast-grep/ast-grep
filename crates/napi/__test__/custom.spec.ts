import test from 'ava'

import {
  registerDynamicLanguage,
  parse,
} from '../index'

const { platform, arch } = process

const isAppleSilicon = platform === 'darwin' && arch === 'arm64'
const isX64Linux = platform === 'linux' && arch === 'x64'
const canTestDynamicLang = isAppleSilicon || isX64Linux

if (isAppleSilicon) {
  registerDynamicLanguage({
    json: {
      libraryPath: "../../benches/fixtures/json-mac.so",
      languageSymbol: "tree_sitter_json",
      extensions: ["json"],
    }
  })
} else if (isX64Linux) {
  registerDynamicLanguage({
    json: {
      libraryPath: "../../benches/fixtures/json-linux.so",
      languageSymbol: "tree_sitter_json",
      extensions: ["json"],
    }
  })
}

test('test load custom lang', t => {
  if (!canTestDynamicLang) {
    t.pass('This test is not available on this platform')
    return
  }
  const sg = parse('json', '{"test": 123}')
  const root = sg.root()
  const node = root.find("123")!
  t.truthy(node)
  t.is(node.kind(), 'number')
  const no = root.find("456")
  t.falsy(no)
})