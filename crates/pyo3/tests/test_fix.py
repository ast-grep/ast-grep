from ast_grep_py import SgRoot

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

def test_one_fix():
    edit = node1.replace("let b = 456")
    r = node1.range()
    assert edit.start_pos == r.start.index
    assert edit.end_pos == r.end.index
    s = node1.commit_edits([edit])
    assert s == "let b = 456"
    s = root.commit_edits([edit])
    assert s == "function test() {\n  let b = 456\n}"

def test_multiple_fix():
    sg = SgRoot('いいよ = log(123) + log(456)', "javascript")
    root = sg.root()
    nodes = root.find_all(kind="number")
    edits = [node.replace('114514') for node in nodes]
    edits = sorted(edits, key=lambda x: x.start_pos, reverse=True)
    s = root.commit_edits(edits)
    assert s == "いいよ = log(114514) + log(114514)"

def test_change_unicode():
    sg = SgRoot('いいよ = log(こいよ)', "javascript")
    root = sg.root()
    nodes = root.find_all(kind="identifier")
    edits = [node.replace('114514') for node in nodes]
    s = root.commit_edits(edits)
    assert s == "114514 = 114514(114514)"

def test_modified_fix():
    sg = SgRoot('いいよ = log(514)', "javascript")
    root = sg.root()
    node = root.find(kind="number")
    assert node
    edit = node.replace('こいよ')
    edit.start_pos -= 1
    edit.end_pos += 1
    s = root.commit_edits([edit])
    assert s == "いいよ = logこいよ"