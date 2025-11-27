//! This mod maintains a list of tree-sitter parsers crate.
//! When feature flag is on, this mod will import all dependent crates.
//! However, tree-sitter bs cannot be compiled by wasm-pack.
//! In this case, we can use a blank implementation by turning feature flag off.
//! And use other implementation.

macro_rules! conditional_lang {
  ($lang: ident, $flag: literal, $field: ident) => {{
    #[cfg(feature=$flag)]
    {
      $lang::$field.into()
    }
    #[cfg(not(feature=$flag))]
    {
      unimplemented!("tree-sitter parser is not implemented when feature flag is off.")
    }
  }};
  ($lang: ident, $flag: literal) => {
    conditional_lang!($lang, $flag, LANGUAGE)
  };
}

use ast_grep_core::tree_sitter::TSLanguage;

pub fn language_bash() -> TSLanguage {
  conditional_lang!(tree_sitter_bash, "tree-sitter-bash")
}
pub fn language_c() -> TSLanguage {
  conditional_lang!(tree_sitter_c, "tree-sitter-c")
}
pub fn language_cpp() -> TSLanguage {
  conditional_lang!(tree_sitter_cpp, "tree-sitter-cpp")
}
pub fn language_c_sharp() -> TSLanguage {
  conditional_lang!(tree_sitter_c_sharp, "tree-sitter-c-sharp")
}
pub fn language_css() -> TSLanguage {
  conditional_lang!(tree_sitter_css, "tree-sitter-css")
}
pub fn language_elixir() -> TSLanguage {
  conditional_lang!(tree_sitter_elixir, "tree-sitter-elixir")
}
pub fn language_go() -> TSLanguage {
  conditional_lang!(tree_sitter_go, "tree-sitter-go")
}
pub fn language_haskell() -> TSLanguage {
  conditional_lang!(tree_sitter_haskell, "tree-sitter-haskell")
}
pub fn language_hcl() -> TSLanguage {
  conditional_lang!(tree_sitter_hcl, "tree-sitter-hcl")
}
pub fn language_html() -> TSLanguage {
  conditional_lang!(tree_sitter_html, "tree-sitter-html")
}
pub fn language_java() -> TSLanguage {
  conditional_lang!(tree_sitter_java, "tree-sitter-java")
}
pub fn language_javascript() -> TSLanguage {
  conditional_lang!(tree_sitter_javascript, "tree-sitter-javascript")
}
pub fn language_json() -> TSLanguage {
  conditional_lang!(tree_sitter_json, "tree-sitter-json")
}
pub fn language_kotlin() -> TSLanguage {
  conditional_lang!(tree_sitter_kotlin, "tree-sitter-kotlin")
}
pub fn language_lua() -> TSLanguage {
  conditional_lang!(tree_sitter_lua, "tree-sitter-lua")
}
pub fn language_nix() -> TSLanguage {
  conditional_lang!(tree_sitter_nix, "tree-sitter-nix")
}
pub fn language_php() -> TSLanguage {
  conditional_lang!(tree_sitter_php, "tree-sitter-php", LANGUAGE_PHP_ONLY)
}
pub fn language_python() -> TSLanguage {
  conditional_lang!(tree_sitter_python, "tree-sitter-python")
}
pub fn language_ruby() -> TSLanguage {
  conditional_lang!(tree_sitter_ruby, "tree-sitter-ruby")
}
pub fn language_rust() -> TSLanguage {
  conditional_lang!(tree_sitter_rust, "tree-sitter-rust")
}
pub fn language_scala() -> TSLanguage {
  conditional_lang!(tree_sitter_scala, "tree-sitter-scala")
}
pub fn language_solidity() -> TSLanguage {
  conditional_lang!(tree_sitter_solidity, "tree-sitter-solidity")
}
pub fn language_swift() -> TSLanguage {
  conditional_lang!(tree_sitter_swift, "tree-sitter-swift")
}
pub fn language_tsx() -> TSLanguage {
  conditional_lang!(
    tree_sitter_typescript,
    "tree-sitter-typescript",
    LANGUAGE_TSX
  )
}
pub fn language_typescript() -> TSLanguage {
  conditional_lang!(
    tree_sitter_typescript,
    "tree-sitter-typescript",
    LANGUAGE_TYPESCRIPT
  )
}
pub fn language_yaml() -> TSLanguage {
  conditional_lang!(tree_sitter_yaml, "tree-sitter-yaml")
}
