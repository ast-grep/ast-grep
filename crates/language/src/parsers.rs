//! This mod maintains a list of tree-sitter parsers crate.
//! When feature flag `builtin-parser` is on, this mod will import all dependent crates.
//! However, tree-sitter bs cannot be compiled by wasm-pack.
//! In this case, we can use a blank implementation by turning feature flag off.
//! And use other implementation.

#[cfg(feature = "builtin-parser")]
macro_rules! into_lang {
  ($lang: ident, $field: ident) => {
    $lang::$field.into()
  };
  ($lang: ident) => {
    into_lang!($lang, LANGUAGE)
  };
}

#[cfg(not(feature = "builtin-parser"))]
macro_rules! into_lang {
  ($lang: ident, $field: ident) => {
    unimplemented!(
      "tree-sitter parser is not implemented when feature flag [builtin-parser] is off."
    )
  };
  ($lang: ident) => {
    into_lang!($lang, LANGUAGE)
  };
}

#[cfg(any(feature = "builtin-parser", feature = "napi-lang"))]
macro_rules! into_napi_lang {
  ($lang: path) => {
    $lang.into()
  };
}
#[cfg(not(any(feature = "builtin-parser", feature = "napi-lang")))]
macro_rules! into_napi_lang {
  ($lang: path) => {
    unimplemented!(
      "tree-sitter parser is not implemented when feature flag [builtin-parser] is off."
    )
  };
}

use ast_grep_core::language::TSLanguage;

pub fn language_bash() -> TSLanguage {
  into_lang!(tree_sitter_bash)
}
pub fn language_c() -> TSLanguage {
  into_lang!(tree_sitter_c)
}
pub fn language_cpp() -> TSLanguage {
  into_lang!(tree_sitter_cpp)
}
pub fn language_c_sharp() -> TSLanguage {
  into_lang!(tree_sitter_c_sharp)
}
pub fn language_css() -> TSLanguage {
  into_napi_lang!(tree_sitter_css::LANGUAGE)
}
pub fn language_elixir() -> TSLanguage {
  into_lang!(tree_sitter_elixir)
}
pub fn language_go() -> TSLanguage {
  into_lang!(tree_sitter_go)
}
pub fn language_haskell() -> TSLanguage {
  into_lang!(tree_sitter_haskell)
}
pub fn language_html() -> TSLanguage {
  into_napi_lang!(tree_sitter_html::LANGUAGE)
}
pub fn language_java() -> TSLanguage {
  into_lang!(tree_sitter_java)
}
pub fn language_javascript() -> TSLanguage {
  into_napi_lang!(tree_sitter_javascript::LANGUAGE)
}
pub fn language_json() -> TSLanguage {
  into_lang!(tree_sitter_json)
}
pub fn language_kotlin() -> TSLanguage {
  into_lang!(tree_sitter_kotlin)
}
pub fn language_lua() -> TSLanguage {
  into_lang!(tree_sitter_lua)
}
pub fn language_php() -> TSLanguage {
  into_lang!(tree_sitter_php, LANGUAGE_PHP_ONLY)
}
pub fn language_python() -> TSLanguage {
  into_lang!(tree_sitter_python)
}
pub fn language_ruby() -> TSLanguage {
  into_lang!(tree_sitter_ruby)
}
pub fn language_rust() -> TSLanguage {
  into_lang!(tree_sitter_rust)
}
pub fn language_scala() -> TSLanguage {
  into_lang!(tree_sitter_scala)
}
pub fn language_swift() -> TSLanguage {
  into_lang!(tree_sitter_swift)
}
pub fn language_tsx() -> TSLanguage {
  into_napi_lang!(tree_sitter_typescript::LANGUAGE_TSX)
}
pub fn language_typescript() -> TSLanguage {
  into_napi_lang!(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)
}
pub fn language_yaml() -> TSLanguage {
  into_lang!(tree_sitter_yaml)
}
