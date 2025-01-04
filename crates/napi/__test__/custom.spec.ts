import test from 'ava'

import {
  registerDynamicLanguage,
  parse,
} from '../index'

const { platform, arch } = process

const isAppleSilicon = platform === 'darwin' && arch === 'arm64'
if (isAppleSilicon) {
  registerDynamicLanguage({
    myjson: {
      libraryPath: "../../benches/fixtures/json-mac.so",
      languageSymbol: "tree_sitter_json",
      extensions: ["myjson"],
    }
  })
}

test('test load custom lang', t => {
  if (!isAppleSilicon) {
    t.pass('This test is not available on this platform')
    return
  }
  // @ts-expect-error TODO: change type
  const sg = parse('myjson', '{"test": 123}')
  const root = sg.root()
  const node = root.find("123")!
  t.truthy(node)
  t.is(node.kind(), 'number')
  const no = root.find("456")
  t.falsy(no)
})