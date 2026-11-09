//! This module defines the supported programming languages for ast-grep.
//!
//! It provides a set of customized languages with expando_char / pre_process_pattern,
//! and a set of stub languages without preprocessing.
//! A rule of thumb: if your language does not accept identifiers like `$VAR`.
//! Add a record to the table in `registry.rs`, with an `expando` character for
//! those languages; omit it for languages that accept `$`. Each record supplies
//! its parser, feature, aliases, extensions, and optional module for tests or
//! custom behavior. Keep the table in alphabetical order.
//! To see the full list of languages, visit `<https://ast-grep.github.io/reference/languages.html>`
//!
//! ```
//! # #[cfg(feature = "tree-sitter-rust")]
//! # {
//! use ast_grep_language::{LanguageExt, SupportLang};
//!
//! let lang: SupportLang = "rs".parse().unwrap();
//! let src = "fn foo() {}";
//! let root = lang.ast_grep(src);
//! let found = root.root().find_all("fn $FNAME() {}").next().unwrap();
//! assert_eq!(found.start_pos().line(), 0);
//! assert_eq!(found.text(), "fn foo() {}");
//! # }
//! ```

#[macro_use]
mod macros;
#[macro_use]
mod registry;

use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};

use ast_grep_core::Node;
use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::tree_sitter::{StrDoc, TSLanguage, TSRange};
use ignore::types::{Types, TypesBuilder};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::borrow::Cow;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;

pub use ast_grep_core::language::Language;
pub use ast_grep_core::tree_sitter::LanguageExt;

pub trait Alias: Display {
  const ALIAS: &'static [&'static str];
}

with_languages!(emit_registry);

impl SupportLang {
  pub fn file_types(&self) -> Types {
    file_types(*self)
  }
}

impl fmt::Display for SupportLang {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{self:?}")
  }
}

#[derive(Debug)]
pub enum SupportLangErr {
  LanguageNotSupported(String),
}

impl Display for SupportLangErr {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
    use SupportLangErr::*;
    match self {
      LanguageNotSupported(lang) => write!(f, "{lang} is not supported!"),
    }
  }
}

impl std::error::Error for SupportLangErr {}

impl<'de> Deserialize<'de> for SupportLang {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_str(SupportLangVisitor)
  }
}

struct SupportLangVisitor;

impl Visitor<'_> for SupportLangVisitor {
  type Value = SupportLang;

  fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str("SupportLang")
  }

  fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    v.parse().map_err(de::Error::custom)
  }
}

/// Implements the language names and aliases.
impl FromStr for SupportLang {
  type Err = SupportLangErr;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    for &lang in Self::all_langs() {
      for moniker in alias(lang) {
        if s.eq_ignore_ascii_case(moniker) {
          return Ok(lang);
        }
      }
    }
    Err(SupportLangErr::LanguageNotSupported(s.to_string()))
  }
}

macro_rules! impl_lang_method {
  ($method: ident, ($($pname:tt: $ptype:ty),*) => $return_type: ty) => {
    #[inline]
    fn $method(&self, $($pname: $ptype),*) -> $return_type {
      with_languages!(emit_dispatch, self, $method, ($($pname),*))
    }
  };
}
impl Language for SupportLang {
  impl_lang_method!(kind_to_id, (_kind: &str) => u16);
  impl_lang_method!(field_to_id, (_field: &str) => Option<u16>);
  impl_lang_method!(meta_var_char, () => char);
  impl_lang_method!(expando_char, () => char);
  impl_lang_method!(extract_meta_var, (_source: &str) => Option<MetaVariable>);
  impl_lang_method!(build_pattern, (_builder: &PatternBuilder) => Result<Pattern, PatternError>);
  fn pre_process_pattern<'q>(&self, _query: &'q str) -> Cow<'q, str> {
    with_languages!(emit_dispatch, self, pre_process_pattern, (_query))
  }
  fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
    from_extension(path.as_ref())
  }
}

impl LanguageExt for SupportLang {
  impl_lang_method!(get_ts_language, () => TSLanguage);
  impl_lang_method!(injectable_languages, () => Option<&'static [&'static str]>);
  fn extract_injections<L: LanguageExt>(
    &self,
    _root: Node<StrDoc<L>>,
  ) -> Vec<(String, Vec<TSRange>)> {
    with_languages!(emit_dispatch, self, extract_injections, (_root))
  }
}

/// Guess which programming language a file is written in
/// Adapt from `<https://github.com/Wilfred/difftastic/blob/master/src/parse/guess_language.rs>`
/// N.B do not confuse it with `FromStr` trait. This function is to guess language from file extension.
fn from_extension(path: &Path) -> Option<SupportLang> {
  let ext = path.extension()?.to_str()?;
  SupportLang::all_langs()
    .iter()
    .copied()
    .find(|&l| extensions(l).contains(&ext))
}

fn add_custom_file_type<'b>(
  builder: &'b mut TypesBuilder,
  file_type: &str,
  suffix_list: &[&str],
) -> &'b mut TypesBuilder {
  for suffix in suffix_list {
    let glob = format!("*.{suffix}");
    builder
      .add(file_type, &glob)
      .expect("file pattern must compile");
  }
  builder.select(file_type)
}

fn file_types(lang: SupportLang) -> Types {
  let mut builder = TypesBuilder::new();
  let exts = extensions(lang);
  let lang_name = lang.to_string();
  add_custom_file_type(&mut builder, &lang_name, exts);
  builder.build().expect("file type must be valid")
}

pub fn config_file_type() -> Types {
  let mut builder = TypesBuilder::new();
  let builder = add_custom_file_type(&mut builder, "yml", &["yml", "yaml"]);
  builder.build().expect("yaml type must be valid")
}

#[cfg(all(test, feature = "builtin-parser"))]
mod serde_test_support;

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::{Pattern, matcher::MatcherExt};

  #[allow(
    dead_code,
    reason = "Parser features select different subsets of test helpers."
  )]
  pub fn test_match_lang(query: &str, source: &str, lang: impl LanguageExt) {
    let cand = lang.ast_grep(source);
    let pattern = Pattern::new(query, lang);
    assert!(
      pattern.find_node(cand.root()).is_some(),
      "goal: {pattern:?}, candidate: {}",
      cand.root().get_inner_node().to_sexp(),
    );
  }

  #[allow(
    dead_code,
    reason = "Parser features select different subsets of test helpers."
  )]
  pub fn test_non_match_lang(query: &str, source: &str, lang: impl LanguageExt) {
    let cand = lang.ast_grep(source);
    let pattern = Pattern::new(query, lang);
    assert!(
      pattern.find_node(cand.root()).is_none(),
      "goal: {pattern:?}, candidate: {}",
      cand.root().get_inner_node().to_sexp(),
    );
  }

  #[allow(
    dead_code,
    reason = "Parser features select different subsets of test helpers."
  )]
  pub fn test_replace_lang(
    src: &str,
    pattern: &str,
    replacer: &str,
    lang: impl LanguageExt,
  ) -> String {
    let mut source = lang.ast_grep(src);
    assert!(
      source
        .replace(pattern, replacer)
        .expect("should parse successfully")
    );
    source.generate()
  }

  #[cfg(feature = "tree-sitter-javascript")]
  #[test]
  fn test_js_string() {
    test_match_lang("'a'", "'a'", JavaScript);
    test_match_lang("\"\"", "\"\"", JavaScript);
    test_match_lang("''", "''", JavaScript);
  }

  #[cfg(feature = "tree-sitter-rust")]
  #[test]
  fn test_enabled_language() {
    let lang: SupportLang = "rs".parse().unwrap();
    assert_eq!(lang, Rust.into());
    assert_eq!(SupportLang::from_path("example.rs"), Some(lang));
    let grep = lang.ast_grep("fn hello() {}");
    assert!(grep.root().find("fn $NAME() {}").is_some());
  }

  #[cfg(feature = "tree-sitter-typescript")]
  #[test]
  fn test_shared_parser_feature() {
    let ts: SupportLang = "ts".parse().unwrap();
    let tsx: SupportLang = "tsx".parse().unwrap();
    assert_eq!(ts, TypeScript.into());
    assert_eq!(tsx, Tsx.into());
    assert_ne!(ts, tsx);
    assert_eq!(SupportLang::from_path("example.mts"), Some(ts));
    assert_eq!(SupportLang::from_path("example.tsx"), Some(tsx));
    for lang in [ts, tsx] {
      let grep = lang.ast_grep("function hello() {}");
      assert!(grep.root().find("function $NAME() {}").is_some());
    }
  }

  #[cfg(feature = "tree-sitter-html")]
  #[test]
  fn test_custom_language_dispatch() {
    let lang: SupportLang = "html".parse().unwrap();
    assert_eq!(lang, Html.into());
    let grep = lang.ast_grep("<script>hello()</script><style>.hello {}</style>");
    let injections = lang.extract_injections(grep.root());
    assert_eq!(injections.len(), 2);
    assert_eq!(injections[0].0, "js");
    assert_eq!(injections[1].0, "css");
  }

  #[test]
  fn test_registry_roundtrip() {
    for &lang in SupportLang::all_langs() {
      for name in alias(lang) {
        assert_eq!(name.parse::<SupportLang>().ok(), Some(lang));
        assert_eq!(name.to_uppercase().parse::<SupportLang>().ok(), Some(lang));
      }
      let types = lang.file_types();
      for ext in extensions(lang) {
        let path = format!("example.{ext}");
        assert_eq!(from_extension(Path::new(&path)), Some(lang));
        assert!(types.matched(Path::new(&path), false).is_whitelist());
      }
    }
  }

  #[cfg(feature = "builtin-parser")]
  #[test]
  fn test_alphabetical_variant_indices() {
    use serde_test_support::IndexSerializer;

    for (lang, index, name) in [
      (SupportLang::Dart, 5, "Dart"),
      (SupportLang::Elixir, 6, "Elixir"),
      (SupportLang::Go, 7, "Go"),
      (SupportLang::Haskell, 8, "Haskell"),
    ] {
      assert_eq!(lang as u32, index);
      assert_eq!(lang.serialize(IndexSerializer).unwrap(), (index, name));
    }

    // Variant indices intentionally follow alphabetical enumeration order.
    const LANGS: &[SupportLang] = SupportLang::all_langs();
    let names: Vec<_> = LANGS
      .iter()
      .map(|lang| lang.to_string().to_ascii_lowercase())
      .collect();
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
      &LANGS[5..9],
      &[
        SupportLang::Dart,
        SupportLang::Elixir,
        SupportLang::Go,
        SupportLang::Haskell,
      ]
    );
  }

  #[cfg(not(feature = "tree-sitter-kotlin"))]
  #[test]
  fn test_disabled_language() {
    for name in ["kotlin", "kt", "Kotlin", "KT"] {
      assert!(
        matches!(name.parse::<SupportLang>(), Err(SupportLangErr::LanguageNotSupported(rejected)) if rejected == name)
      );
    }
    for ext in ["kt", "ktm", "kts"] {
      assert_eq!(SupportLang::from_path(format!("example.{ext}")), None);
    }
  }
}
