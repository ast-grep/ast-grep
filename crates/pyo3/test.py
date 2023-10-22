from ast_grep_pyo3 import SgRoot

source = '''
function test() {
  let a = 123
}
'''.strip()

sg = SgRoot(source, 'javascript')
root = sg.root()

node = root.find(pattern = 'let $A = $B')
assert node is not None
node2 = root.find(pattern = 'let $A = 123')

node = root.find(dict(
  rule=dict(pattern = 'let $A = 123')
))
assert node is not None