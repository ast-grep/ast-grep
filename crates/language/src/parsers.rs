//! This mod maintains a list of tree-sitter parsers crate.
//! Each language parser is gated behind its own feature flag.
//! When a language feature is enabled, the corresponding tree-sitter parser is imported.
//! When a language feature is disabled, the function returns unimplemented!().

use ast_grep_core::tree_sitter::TSLanguage;

macro_rules! define_lang_parser {
  ($func:ident, $feature:literal, $lang:ident) => {
    #[cfg(feature = $feature)]
    #[allow(dead_code)]
    pub fn $func() -> TSLanguage {
      $lang::LANGUAGE.into()
    }
    #[cfg(not(feature = $feature))]
    #[allow(dead_code)]
    pub fn $func() -> TSLanguage {
      unimplemented!(
        "tree-sitter parser for {} is not available. Enable the '{}' feature to use it.",
        stringify!($func),
        $feature
      )
    }
  };
  ($func:ident, $feature:literal, $lang:ident, $field:ident) => {
    #[cfg(feature = $feature)]
    #[allow(dead_code)]
    pub fn $func() -> TSLanguage {
      $lang::$field.into()
    }
    #[cfg(not(feature = $feature))]
    #[allow(dead_code)]
    pub fn $func() -> TSLanguage {
      unimplemented!(
        "tree-sitter parser for {} is not available. Enable the '{}' feature to use it.",
        stringify!($func),
        $feature
      )
    }
  };
}

define_lang_parser!(language_bash, "lang-bash", tree_sitter_bash);
define_lang_parser!(language_c, "lang-c", tree_sitter_c);
define_lang_parser!(language_cpp, "lang-cpp", tree_sitter_cpp);
define_lang_parser!(language_c_sharp, "lang-csharp", tree_sitter_c_sharp);
define_lang_parser!(language_css, "lang-css", tree_sitter_css);
define_lang_parser!(language_elixir, "lang-elixir", tree_sitter_elixir);
define_lang_parser!(language_go, "lang-go", tree_sitter_go);
define_lang_parser!(language_haskell, "lang-haskell", tree_sitter_haskell);
define_lang_parser!(language_hcl, "lang-hcl", tree_sitter_hcl);
define_lang_parser!(language_html, "lang-html", tree_sitter_html);
define_lang_parser!(language_java, "lang-java", tree_sitter_java);
define_lang_parser!(language_javascript, "lang-javascript", tree_sitter_javascript);
define_lang_parser!(language_json, "lang-json", tree_sitter_json);
define_lang_parser!(language_kotlin, "lang-kotlin", tree_sitter_kotlin);
define_lang_parser!(language_lua, "lang-lua", tree_sitter_lua);
define_lang_parser!(language_nix, "lang-nix", tree_sitter_nix);
define_lang_parser!(language_php, "lang-php", tree_sitter_php, LANGUAGE_PHP_ONLY);
define_lang_parser!(language_python, "lang-python", tree_sitter_python);
define_lang_parser!(language_ruby, "lang-ruby", tree_sitter_ruby);
define_lang_parser!(language_rust, "lang-rust", tree_sitter_rust);
define_lang_parser!(language_scala, "lang-scala", tree_sitter_scala);
define_lang_parser!(language_solidity, "lang-solidity", tree_sitter_solidity);
define_lang_parser!(language_swift, "lang-swift", tree_sitter_swift);
define_lang_parser!(language_tsx, "lang-typescript", tree_sitter_typescript, LANGUAGE_TSX);
define_lang_parser!(language_typescript, "lang-typescript", tree_sitter_typescript, LANGUAGE_TYPESCRIPT);
define_lang_parser!(language_yaml, "lang-yaml", tree_sitter_yaml);
