from ast_grep_pyo3 import SgRoot

source = '''
function test() {
  let a = 123
}
'''.strip()

sg = SgRoot(source, 'javascript')
root = sg.root()
node1 = root.find(pattern = 'let $A = $B')
node2 = root.find(pattern = 'let $A = 123')

def test_pos():
  r1 = node1.range()
  r2 = node2.range()
  pos = r1.start
  pos2 = r2.start
  assert pos.line == 1
  assert pos.column == 2
  assert pos.index == 20
  assert pos == pos2

def test_range():
  r1 = node1.range()
  r2 = node2.range()
  assert r1.start.line == 1
  assert r1.end.line == 1
  assert r1.start.column == 2
  assert r1.end.column == 13
  assert r1.start.index == 20
  assert r1.end.index == 31
  assert r1 == r2