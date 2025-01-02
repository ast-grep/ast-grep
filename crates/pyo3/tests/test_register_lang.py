from ast_grep_py import SgRoot, register_dynamic_language
import platform

source = """
{
  "test": 123
}
""".strip()

# at the moment only test darwin arm64
def is_arm_mac():
    return platform.system() == 'Darwin' and platform.processor() == 'arm'

def register_lang():
    if not is_arm_mac():
        return
    register_dynamic_language({
        "myjson": {
            "library_path": "../../benches/fixtures/json-mac.so",
            "language_symbol": "tree_sitter_json",
            "extensions": ["myjson"],
        }
    })

register_lang()

def test_load_json_lang():
    if not is_arm_mac():
        return
    sg = SgRoot(source, "myjson")
    root = sg.root()
    node = root.find(pattern="123")
    assert node
    assert node.kind() == "number"
    node = root.find(pattern="456")
    assert not node