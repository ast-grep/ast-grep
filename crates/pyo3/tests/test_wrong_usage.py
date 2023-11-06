from ast_grep_py import SgRoot
import pytest

source = """
function test() {
  let a = 123
  let b = 456
  let c = 789
}
""".strip()
sg = SgRoot(source, "javascript")
root = sg.root()

def test_wrong_use_pattern_as_dict():
    with pytest.raises(TypeError):
        root.find("let $A = 123") # type: ignore

def test_get_unfound_match():
    node = root.find(pattern="let $A = 123")
    assert node is not None
    with pytest.raises(KeyError):
        node["B"]

# TODO: remove BaseException
def test_wrong_rule_key():
    with pytest.raises(Exception):
        root.find(pat="not") # type: ignore

def test_no_rule_key():
    with pytest.raises(ValueError):
        root.matches()
    with pytest.raises(ValueError):
        root.inside()
    with pytest.raises(ValueError):
        root.has()
    with pytest.raises(ValueError):
        root.follows()
    with pytest.raises(ValueError):
        root.follows()

def test_error_for_invalid_kind():
    with pytest.raises(RuntimeError):
        root.find(kind="nonsense")

def test_no_error_for_invalid_pattern():
    with pytest.raises(RuntimeError):
        root.find(pattern="$@!!--l3**+no//nsense")
    # but not this
    assert not root.find(pattern="@test")