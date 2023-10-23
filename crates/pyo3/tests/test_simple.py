from ast_grep_pyo3 import SgRoot

source = '''
function test() {
  let a = 123
}
'''.strip()

def test_simple():
  sg = SgRoot(source, 'javascript')
  root = sg.root()

  node = root.find(pattern = 'let $A = $B')
  assert node is not None

  node = root.find(dict(
    rule=dict(pattern = 'let $A = 123')
  ))
  assert node is not None