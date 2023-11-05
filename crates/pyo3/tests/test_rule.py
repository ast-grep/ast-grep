from ast_grep_pyo3 import SgRoot, Rule, Config, Relation

source = """
function test() {
  let a = 123
}
""".strip()

sg = SgRoot(source, "javascript")
root = sg.root()

def test_simple():
    node = root.find(pattern="let $A = $B")
    assert node is not None

def test_config():
    node = root.find(
        Config(
            rule={"pattern": "let a = 123"},
        )
    )
    assert node is not None

def test_config_literal():
    node = root.find({
        "rule": {"pattern": "let a = 123"},
    })
    assert node is not None

def test_rule():
    rule = Rule(pattern = "let $A = $B")
    node = root.find(**rule)
    assert node is not None

def test_dict_literal():
    # pyright is not smart to infer dict.
    # We have to annotate Rule here
    rule: Rule = {"pattern": "let $A = $B"}
    node = root.find(**rule)
    assert node is not None

def test_not_rule():
    rule = {"pattern": "let $A = $B", "not": Rule(pattern="let a = 123")}
    node = root.find(**rule)
    assert not node
    rule = {"pattern": "let $A = $B", "not": Rule(pattern="let b = 123")}
    node = root.find(**rule)
    assert node

def test_relational_rule():
    relation: Relation = Relation(kind="function_declaration", stopBy="end")
    node = root.find(
        pattern="let a = 123\n",
        inside=relation,
    )
    assert node