from ast_grep_py import SgRoot, Rule, Config, Relation, Pattern

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

def test_relational_dict():
    relation: Relation = {"kind": "function_declaration", "stopBy": "end"}
    node = root.find(
        pattern="let a = 123\n",
        inside=relation,
    )
    assert node
    node = root.find(
        pattern="let a = 123\n",
        inside={"kind": "function_declaration", "stopBy": "end"},
    )
    assert node

def test_relational_rule():
    node = root.find(
        pattern="let a = 123\n",
        inside=Relation(kind="function_declaration", stopBy="end"),
    )
    assert node

def test_complex_config_dict():
    node = root.find({
        "rule": {
            "pattern": "let $A = $B",
            "regex": "123",
            "not": {
                "regex": "456"
            },
        },
        "constraints": {
            "A": {
                "pattern": "a"
            }
        },
        "transform": {
            "C": {
                "substring": {
                    "source": "$B",
                    "startChar": 1,
                    "endChar": -1,
                }
            }
        }
    })
    assert node
    assert node.get_transformed("C") == "2"

def test_complex_config_dict_not_found():
    node = root.find({
        "rule": {
            "pattern": "let $A = $B",
            "regex": "123",
            "not": {
                "regex": "456"
            },
        },
        "constraints": {
            "A": {
                "pattern": "a"
            },
            "B": {
                "regex": "222"
            },
        },
        "transform": {
            "C": {
                "substring": {
                    "source": "$B",
                    "startChar": 1,
                    "endChar": -1,
                }
            }
        }
    })
    assert not node

def test_complex_config():
    node = root.find(Config(
        rule=Rule(pattern="let $A = $B", regex="123"),
        constraints=dict(A=Rule(pattern="a")),
        transform=dict(C={
            "substring": {
                "source": "$B",
                "startChar": 1,
            }
        })
    ))
    assert node
    assert node.text() == "let a = 123"
    assert node.get_transformed("C") == "23"

def test_pattern():
    node = root.find(pattern={
        "context": "let a = 123",
        "selector": "variable_declarator"
    })
    assert node
    assert node.text() == "a = 123"
    node2 = root.find(pattern=Pattern(
        context="let a = 123",
        selector="variable_declarator",
    ))
    assert node == node2

def test_range_rule():
    node = root.find(range={
        "start": {"line": 0, "column": 9},
        "end": {"line": 0, "column": 13},
    })
    assert node
    assert node.text() == "test"
    node = root.find(range={
        "start": {"line": 0, "column": 9},
        "end": {"line": 0, "column": 12},
    })
    assert not node

def test_strictness():
    node = root.find(pattern={
        "context": "let b = 456",
        "strictness": "signature",
    })