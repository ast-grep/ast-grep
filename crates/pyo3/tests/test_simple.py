from ast_grep_py import SgRoot, Rule

source = """
function test() {
  let a = 123
  let b = 456
  let c = 789
}
""".strip()
sg = SgRoot(source, "javascript")
root = sg.root()

def test_is_leaf():
    node = root.find(pattern="let $A = $B")
    assert node
    assert not node.is_leaf()
    node = root.find(pattern="123")
    assert node
    assert node.is_leaf()

def test_is_named():
    node = root.find(pattern="let $A = $B")
    assert node
    assert node.is_named()
    node = root.find(pattern="123")
    assert node
    assert node.is_named()

def test_kind():
    node = root.find(pattern="let $A = $B")
    assert node
    assert node.kind() == "lexical_declaration"
    node = root.find(pattern="123")
    assert node
    assert node.kind() == "number"

def test_text():
    node = root.find(pattern="let $A = $B")
    assert node
    assert node.text() == "let a = 123"
    node = root.find(kind="number")
    assert node
    assert node.text() == "123"

def test_matches():
    node = root.find(pattern="let $A = $B")
    assert node
    assert node.matches(kind="lexical_declaration")
    assert not node.matches(kind="number")
    assert node.matches(pattern="let a = 123")
    assert not node.matches(pattern="let b = 456")
    assert node.matches(has=Rule(
        kind="variable_declarator",
        has=Rule(
            kind="number",
            pattern="123"
        )
    ))

def test_inside():
    node = root.find(pattern="let $A = $B")
    assert node
    assert node.inside(kind="function_declaration")
    assert not node.inside(kind="function_expression")

def test_has():
    node = root.find(pattern="let $A = $B")
    assert node
    assert node.has(pattern="123")
    assert node.has(kind="number")
    assert not node.has(kind="function_expression")

def test_precedes():
    node = root.find(pattern="let $A = $B\n")
    assert node
    assert node.precedes(pattern="let b = 456\n")
    assert node.precedes(pattern="let c = 789\n")
    assert not node.precedes(pattern="notExist")

def test_follows():
    node = root.find(pattern="let b = 456\n")
    assert node
    assert node.follows(pattern="let a = 123\n")
    assert not node.follows(pattern="let c = 789\n")

def test_get_match():
    node = root.find(pattern="let $A = $B")
    assert node
    a = node.get_match("A")
    assert a is not None
    assert a.text() == "a"
    rng = a.range()
    assert rng.start.line == 1
    assert rng.start.column == 6

def test_must_get_match():
    node = root.find(pattern="let $A = $B")
    assert node
    a = node["A"]
    assert a is not None
    assert a.text() == "a"
    rng = a.range()
    assert rng.start.line == 1
    assert rng.start.column == 6


def test_get_multi_match():
    node = root.find(pattern="function test() { $$$STMT }")
    assert node
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