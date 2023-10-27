from ast_grep_pyo3 import SgRoot

source = """
function test() {
  let a = 123
  let b = 456
  let c = 789
}
""".strip()
sg = SgRoot(source, "javascript")
root = sg.root()


def test_simple():
    node = root.find(pattern="let $A = $B")
    assert node is not None
    node = root.find(
        dict(
            rule=dict(pattern="let $A = 123"),
        )
    )
    assert node is not None

def test_inspection():
    pass

def test_matches():
    pass

def test_inside():
    pass

def test_has():
    pass

def test_precedes():
    pass

def test_follows():
    pass

def test_get_match():
    node = root.find(pattern="let $A = $B")
    a = node.get_match("A")
    assert a is not None
    assert a.text() == "a"
    rng = a.range()
    assert rng.start.line == 1
    assert rng.start.column == 6


def test_get_multi_match():
    node = root.find(pattern="function test() { $$$STMT }")
    stmts = node.get_multiple_matches("STMT")
    assert len(stmts) == 3
    assert stmts[0] == root.find(pattern="let a = 123")

def test_hash():
    node1 = root.find(pattern="let $A = $B")
    node2 = root.find(pattern="let $A = 123")
    assert hash(node1) == hash(node2)

def test_eq():
    node1 = root.find(pattern="let $A = $B")
    node2 = root.find(pattern="let $A = 123")
    assert node1 == node2

def test_str():
    node1 = root.find(pattern="let $A = $B")
    assert str(node1) == "lexical_declaration@(1,2)-(1,13)"

def test_repr_short():
    node1 = root.find(pattern="let $A = $B")
    assert repr(node1) == "SgNode(`let a...`, kind=lexical_declaration, range=(1,2)-(1,13))"

def test_repr_long():
    node1 = root.find(pattern="123")
    assert repr(node1) == "SgNode(`123`, kind=number, range=(1,10)-(1,13))"