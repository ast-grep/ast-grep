//! This mod maintains a list of tree-sitter parsers crate.
//! When feature flag `builtin-parser` is on, this mod will import all dependent crates.
//! However, tree-sitter bs cannot be compiled by wasm-pack.
//! In this case, we can use a blank implementation by turning feature flag off.
//! And use other implementation.

#[cfg(feature = "builtin-parser")]
mod parser_implmentation {
  use ast_grep_core::language::TSLanguage;

  pub fn language_c() -> TSLanguage {
    tree_sitter_c::language().into()
  }
  pub fn language_cpp() -> TSLanguage {
    tree_sitter_cpp::language().into()
  }
  pub fn language_c_sharp() -> TSLanguage {
    tree_sitter_c_sharp::language().into()
  }
  pub fn language_css() -> TSLanguage {
    tree_sitter_css::language().into()
  }
  pub fn language_dart() -> TSLanguage {
    tree_sitter_dart::language().into()
  }
  pub fn language_go() -> TSLanguage {
    tree_sitter_go::language().into()
  }
  pub fn language_html() -> TSLanguage {
    tree_sitter_html::language().into()
  }
  pub fn language_java() -> TSLanguage {
    tree_sitter_java::language().into()
  }
  pub fn language_javascript() -> TSLanguage {
    tree_sitter_javascript::language().into()
  }
  pub fn language_json() -> TSLanguage {
    tree_sitter_json::language().into()
  }
  pub fn language_kotlin() -> TSLanguage {
    tree_sitter_kotlin::language().into()
  }
  pub fn language_lua() -> TSLanguage {
    tree_sitter_lua::language().into()
  }
  pub fn language_python() -> TSLanguage {
    tree_sitter_python::language().into()
  }
  pub fn language_ruby() -> TSLanguage {
    tree_sitter_ruby::language().into()
  }
  pub fn language_rust() -> TSLanguage {
    tree_sitter_rust::language().into()
  }
  pub fn language_scala() -> TSLanguage {
    tree_sitter_scala::language().into()
  }
  pub fn language_swift() -> TSLanguage {
    tree_sitter_swift::language().into()
  }
  pub fn language_thrift() -> TSLanguage {
    tree_sitter_thrift::language().into()
  }
  pub fn language_tsx() -> TSLanguage {
    tree_sitter_typescript::language_tsx().into()
  }
  pub fn language_typescript() -> TSLanguage {
    tree_sitter_typescript::language_typescript().into()
  }
}

#[cfg(not(feature = "builtin-parser"))]
mod parser_implmentation {
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
    language_c,
    language_cpp,
    language_c_sharp,
    language_css,
    language_dart,
    language_go,
    language_html,
    language_java,
    language_javascript,
    language_json,
    language_kotlin,
    language_lua,
    language_python,
    language_ruby,
    language_rust,
    language_scala,
    language_swift,
    language_thrift,
    language_tsx,
    language_typescript,
  );
}

pub use parser_implmentation::*;
