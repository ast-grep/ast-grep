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

def is_x64_linux():
    return platform.system() == 'Linux' and platform.machine() == 'x86_64'

def register_lang():
    if is_arm_mac():
        register_dynamic_language({
            "myjson": {
                "library_path": "../../fixtures/json-mac.so",
                "language_symbol": "tree_sitter_json",
                "extensions": ["myjson"],
            }
        })
    if is_x64_linux():
        register_dynamic_language({
            "myjson": {
                "library_path": "../../fixtures/json-linux.so",
                "language_symbol": "tree_sitter_json",
                "extensions": ["myjson"],
            }
        })

register_lang()

def test_load_json_lang():
    if not is_arm_mac() and not is_x64_linux():
        return
    sg = SgRoot(source, "myjson")
    root = sg.root()
    node = root.find(pattern="123")
    assert node
    assert node.kind() == "number"
    node = root.find(pattern="456")
    assert not node