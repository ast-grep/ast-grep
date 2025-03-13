macro_rules! into_lang {
  ($lang: ident, $field: ident) => {
    $lang::$field.into()
  };
  ($lang: ident) => {
    into_lang!($lang, LANGUAGE)
  };
}

macro_rules! into_unimplemented_lang {
  ($lang: ident, $field: ident) => {
    unimplemented!("This parser is not supported with feature [napi-lang].")
  };
  ($lang: ident) => {
    unimplemented!("This parser is not supported with feature [napi-lang].")
  };
}

#[allow(dead_code)]
pub mod parsers {
  use ast_grep_core::language::TSLanguage;

  pub fn language_bash() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_bash)
  }
  pub fn language_c() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_c)
  }
  pub fn language_cpp() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_cpp)
  }
  pub fn language_c_sharp() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_c_sharp)
  }
  pub fn language_css() -> TSLanguage {
    into_lang!(tree_sitter_css, LANGUAGE)
  }
  pub fn language_elixir() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_elixir)
  }
  pub fn language_go() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_go)
  }
  pub fn language_haskell() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_haskell)
  }
  pub fn language_html() -> TSLanguage {
    into_lang!(tree_sitter_html, LANGUAGE)
  }
  pub fn language_java() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_java)
  }
  pub fn language_javascript() -> TSLanguage {
    into_lang!(tree_sitter_javascript, LANGUAGE)
  }
  pub fn language_json() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_json)
  }
  pub fn language_kotlin() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_kotlin)
  }
  pub fn language_lua() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_lua)
  }
  pub fn language_php() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_php, LANGUAGE_PHP_ONLY)
  }
  pub fn language_python() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_python)
  }
  pub fn language_ruby() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_ruby)
  }
  pub fn language_rust() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_rust)
  }
  pub fn language_scala() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_scala)
  }
  pub fn language_swift() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_swift)
  }
  pub fn language_tsx() -> TSLanguage {
    into_lang!(tree_sitter_typescript, LANGUAGE_TSX)
  }
  pub fn language_typescript() -> TSLanguage {
    into_lang!(tree_sitter_typescript, LANGUAGE_TYPESCRIPT)
  }
  pub fn language_yaml() -> TSLanguage {
    into_unimplemented_lang!(tree_sitter_yaml)
  }
}
