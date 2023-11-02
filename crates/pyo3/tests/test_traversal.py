from ast_grep_pyo3 import SgNode, SgRoot

source = """
function test() {
  let a = 123
  let b = 456
  let c = 789
}
""".strip()
sg = SgRoot(source, "javascript")
root = sg.root()

def test_get_root():
    node = root.find(pattern="let a = $A")
    assert node is not None
    root2 = node.get_root()
    assert root2.filename() == "anonymous"
    # assert root2 == root


def test_find_all():
    nodes = root.find_all(pattern="let $N = $V")
    assert len(nodes) == 3

    def assert_name(node: SgNode, text: str):
        n = node.get_match("N")
        assert n is not None
        assert n.text() == text

    assert_name(nodes[0], "a")
    assert_name(nodes[1], "b")
    assert_name(nodes[2], "c")

def test_field():
    node = root.find(kind="variable_declarator")
    name = node.field("name")
    assert name is not None
    assert name.text() == "a"
    value = node.field("value")
    assert value is not None
    assert value.text() == "123"
    non = node.field("notexist")
    assert non is None

def test_parent():
    node = root.find(kind="variable_declarator")
    parent = node.parent()
    assert parent is not None
    assert parent.kind() == "lexical_declaration"
    assert root.parent() is None

def test_child(): pass

def test_children():
    node = root.find(kind="variable_declarator")
    children = node.children()
    assert len(children) == 3
    assert children[0].text() == "a"
    assert children[2].text() == "123"
    assert not children[0].children()

def test_ancestors(): pass
def test_next(): pass
def test_next_all(): pass
def test_prev(): pass
def test_prev_all(): pass