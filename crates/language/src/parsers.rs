//! This mod maintains a list of tree-sitter parsers crate.
//! When feature flag `builtin-parser` is on, this mod will import all dependent crates.
//! However, tree-sitter bs cannot be compiled by wasm-pack.
//! In this case, we can use a blank implementation by turning feature flag off.
//! And use other implementation.

#[cfg(feature = "builtin-parser")]
mod parser_implementation {
  use ast_grep_core::language::TSLanguage;

  pub fn language_bash() -> TSLanguage {
    tree_sitter_bash::LANGUAGE.into()
  }
  pub fn language_c() -> TSLanguage {
    tree_sitter_c::LANGUAGE.into()
  }
  pub fn language_cpp() -> TSLanguage {
    tree_sitter_cpp::LANGUAGE.into()
  }
  pub fn language_c_sharp() -> TSLanguage {
    tree_sitter_c_sharp::LANGUAGE.into()
  }
  pub fn language_css() -> TSLanguage {
    tree_sitter_css::LANGUAGE.into()
  }
  pub fn language_elixir() -> TSLanguage {
    tree_sitter_elixir::LANGUAGE.into()
  }
  pub fn language_go() -> TSLanguage {
    tree_sitter_go::LANGUAGE.into()
  }
  pub fn language_haskell() -> TSLanguage {
    tree_sitter_haskell::LANGUAGE.into()
  }
  pub fn language_html() -> TSLanguage {
    tree_sitter_html::LANGUAGE.into()
  }
  pub fn language_java() -> TSLanguage {
    tree_sitter_java::LANGUAGE.into()
  }
  pub fn language_javascript() -> TSLanguage {
    tree_sitter_javascript::LANGUAGE.into()
  }
  pub fn language_json() -> TSLanguage {
    tree_sitter_json::LANGUAGE.into()
  }
  pub fn language_kotlin() -> TSLanguage {
    tree_sitter_kotlin::LANGUAGE.into()
  }
  pub fn language_lua() -> TSLanguage {
    tree_sitter_lua::LANGUAGE.into()
  }
  pub fn language_php() -> TSLanguage {
    tree_sitter_php::LANGUAGE_PHP.into()
  }
  pub fn language_python() -> TSLanguage {
    tree_sitter_python::LANGUAGE.into()
  }
  pub fn language_ruby() -> TSLanguage {
    tree_sitter_ruby::LANGUAGE.into()
  }
  pub fn language_rust() -> TSLanguage {
    tree_sitter_rust::LANGUAGE.into()
  }
  pub fn language_scala() -> TSLanguage {
    tree_sitter_scala::LANGUAGE.into()
  }
  pub fn language_sql() -> TSLanguage {
    tree_sitter_sequel::LANGUAGE.into()
  }
  pub fn language_swift() -> TSLanguage {
    tree_sitter_swift::LANGUAGE.into()
  }
  pub fn language_tsx() -> TSLanguage {
    tree_sitter_typescript::LANGUAGE_TSX.into()
  }
  pub fn language_typescript() -> TSLanguage {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
  }
  pub fn language_yaml() -> TSLanguage {
    tree_sitter_yaml::LANGUAGE.into()
  }
}

#[cfg(not(feature = "builtin-parser"))]
mod parser_implementation {
  use ast_grep_core::language::TSLanguage;
  macro_rules! impl_parsers {
    // simple parser for one lang
    ($parser_func: ident) => {
      pub fn $parser_func() -> TSLanguage {
        unimplemented!("tree-sitter parser is not implemented when feature flag [builtin-parser] is off.")
      }
    };
    // repeat
    ($parser_func: ident, $($funcs: ident,)*) => {
      impl_parsers!($parser_func);
      impl_parsers! { $($funcs,)* }
    };
    // terminating condition
    () => {}
  }

  impl_parsers!(
    language_bash,
    language_c,
    language_cpp,
    language_c_sharp,
    language_css,
    language_elixir,
    language_go,
    language_haskell,
    language_html,
    language_java,
    language_javascript,
    language_json,
    language_kotlin,
    language_lua,
    language_php,
    language_python,
    language_ruby,
    language_rust,
    language_scala,
    language_sql,
    language_swift,
    language_tsx,
    language_typescript,
    language_yaml,
  );
}

pub use parser_implementation::*;
