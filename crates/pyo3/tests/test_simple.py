from ast_grep_pyo3 import SgRoot

source = '''
function test() {
  let a = 123
  let b = 456
  let c = 789
}
'''.strip()
sg = SgRoot(source, 'javascript')
root = sg.root()

def test_simple():
  node = root.find(pattern = 'let $A = $B')
  assert node is not None
  node = root.find(dict(
    rule=dict(pattern = 'let $A = 123')
  ))
  assert node is not None


def test_get_match():
  node = root.find(pattern = 'let $A = $B')
  a = node.get_match('A')
  assert a is not None
  assert a.text() == 'a'
  rng = a.range()
  assert rng.start.line == 1
  assert rng.start.column == 6

def test_get_multi_match():
  pass

def test_get_root():
  node = root.find(pattern = 'let a = $A')
  assert node is not None
  root2 = node.get_root()
  assert root2.filename() == 'anonymous'
  # assert root2 == root

def test_get_all():
  nodes = root.find_all(pattern = 'let $N = $V')
  assert len(nodes) == 3
  assert nodes[0].get_match('N').text() == 'a'
  assert nodes[1].get_match('N').text() == 'b'
  assert nodes[2].get_match('N').text() == 'c'