macro_rules! into_unimplemented_lang {
  () => {
    unimplemented!("this parser is not available.")
  };
}

#[allow(dead_code)]
pub mod parsers_none {
  use ast_grep_core::language::TSLanguage;
  pub fn language_bash() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_c() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_cpp() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_c_sharp() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_css() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_elixir() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_go() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_haskell() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_html() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_java() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_javascript() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_json() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_kotlin() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_lua() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_php() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_python() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_ruby() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_rust() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_scala() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_swift() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_tsx() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_typescript() -> TSLanguage {
    into_unimplemented_lang!()
  }
  pub fn language_yaml() -> TSLanguage {
    into_unimplemented_lang!()
  }
}
