from typing import Optional, TypeVar
from ast_grep_py import SgNode, SgRoot

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
    assert node
    name = node.field("name")
    assert name is not None
    assert name.text() == "a"
    value = node.field("value")
    assert value is not None
    assert value.text() == "123"
    non = node.field("notexist")
    assert non is None

def test_field_children():
    source = 'const el = <div id="foo" className="bar" />'
    sg = SgRoot(source, "tsx")
    root = sg.root()
    node = root.find(kind="jsx_self_closing_element")
    assert node
    attributes = node.field_children("attribute")
    assert len(attributes) == 2
    assert attributes[0].text() == 'id="foo"'
    assert attributes[1].text() == 'className="bar"'

def test_parent():
    node = root.find(kind="variable_declarator")
    assert node
    parent = node.parent()
    assert parent is not None
    assert parent.kind() == "lexical_declaration"
    assert root.parent() is None

T = TypeVar('T')
def unwrap(n: Optional[T]) -> T:
    assert n is not None
    return n

def test_child():
    node = root.find(kind="variable_declarator")
    assert node
    assert unwrap(node.child(0)).text() == "a"
    assert unwrap(node.child(2)).text() == "123"
    assert node.child(3) is None

def test_children():
    node = root.find(kind="variable_declarator")
    assert node
    children = node.children()
    assert len(children) == 3
    assert children[0].text() == "a"
    assert children[2].text() == "123"
    assert not children[0].children()

def test_ancestors():
    node = root.find(kind="variable_declarator")
    assert node
    ancestors = node.ancestors()
    assert len(ancestors) == 4
    assert not root.ancestors()

def test_next():
    node = root.find(pattern="let a = $A\n")
    assert node is not None
    neighbor = node.next()
    assert neighbor is not None
    assert neighbor.text() == "let b = 456"
    node = root.find(pattern="let c = $A\n")
    assert node
    node = node.next()
    assert node # `}` is the last node
    assert not node.next()

def test_next_all():
    node = root.find(pattern="let a = $A\n")
    assert node is not None
    next_all = node.next_all()
    assert len(next_all) == 3
    assert len(next_all[0].next_all()) == 2
    assert not next_all[2].next_all()

def test_prev():
    node = root.find(pattern="let c = $A\n")
    assert node is not None
    neighbor = node.prev()
    assert neighbor is not None
    assert neighbor.text() == "let b = 456"
    node = unwrap(root.find(pattern="let a = $A\n")).prev()
    assert node # `{` is the first node
    assert not node.prev()

def test_prev_all():
    node = root.find(pattern="let c = $A\n")
    assert node is not None
    prev_all = node.prev_all()
    assert len(prev_all) == 3
    assert len(prev_all[0].prev_all()) == 2
    assert prev_all[0].text() == "let b = 456"
    assert not prev_all[2].prev_all()