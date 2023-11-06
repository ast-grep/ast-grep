from ast_grep_py import SgRoot, Range, Pos

source = """
function test() {
  let a = 123
}
""".strip()

sg = SgRoot(source, "javascript")
root = sg.root()
node1 = root.find(pattern="let $A = $B")
assert node1 is not None
node2 = root.find(pattern="let $A = 123")
assert node2 is not None


def test_pos():
    r1 = node1.range()
    r2 = node2.range()
    pos = r1.start
    pos2 = r2.start
    assert isinstance(pos, Pos)
    assert pos.line == 1
    assert pos.column == 2
    assert pos.index == 20
    assert pos == pos2
    assert hash(pos) == hash(pos2)


def test_range():
    r1 = node1.range()
    r2 = node2.range()
    assert isinstance(r1, Range)
    assert r1.start.line == 1
    assert r1.end.line == 1
    assert r1.start.column == 2
    assert r1.end.column == 13
    assert r1.start.index == 20
    assert r1.end.index == 31
    assert r1 == r2
    assert hash(r1) == hash(r2)