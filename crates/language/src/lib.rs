//! This module defines the supported programming languages for ast-grep.
//!
//! It provides a set of customized languages with expando_char / pre_process_pattern,
//! and a set of stub languages without preprocessing.
//! A rule of thumb: if your language does not accept identifiers like `$VAR`.
//! You need use `impl_lang_expando!` macro and a standalone file for testing.
//! Otherwise, you can define it as a stub language using `impl_lang!`.
//! To see the full list of languages, visit `<https://ast-grep.github.io/reference/languages.html>`
//!
//! ```
//! use ast_grep_language::{LanguageExt, SupportLang};
//!
//! let lang: SupportLang = "rs".parse().unwrap();
//! let src = "fn foo() {}";
//! let root = lang.ast_grep(src);
//! let found = root.root().find_all("fn $FNAME() {}").next().unwrap();
//! assert_eq!(found.start_pos().line(), 0);
//! assert_eq!(found.text(), "fn foo() {}");
//! ```

#[cfg(feature = "tree-sitter-bash")]
mod bash;
#[cfg(feature = "tree-sitter-cpp")]
mod cpp;
#[cfg(feature = "tree-sitter-c-sharp")]
mod csharp;
#[cfg(feature = "tree-sitter-css")]
mod css;
#[cfg(feature = "tree-sitter-dart")]
mod dart;
#[cfg(feature = "tree-sitter-elixir")]
mod elixir;
#[cfg(feature = "tree-sitter-go")]
mod go;
#[cfg(feature = "tree-sitter-haskell")]
mod haskell;
#[cfg(feature = "tree-sitter-hcl")]
mod hcl;
#[cfg(feature = "tree-sitter-html")]
mod html;
#[cfg(feature = "tree-sitter-json")]
mod json;
#[cfg(feature = "tree-sitter-kotlin")]
mod kotlin;
#[cfg(feature = "tree-sitter-lua")]
mod lua;
#[cfg(feature = "tree-sitter-md")]
mod markdown;
#[cfg(feature = "tree-sitter-nix")]
mod nix;
#[cfg(any(
  feature = "tree-sitter-bash",
  feature = "tree-sitter-c",
  feature = "tree-sitter-c-sharp",
  feature = "tree-sitter-cpp",
  feature = "tree-sitter-css",
  feature = "tree-sitter-dart",
  feature = "tree-sitter-elixir",
  feature = "tree-sitter-go",
  feature = "tree-sitter-haskell",
  feature = "tree-sitter-hcl",
  feature = "tree-sitter-html",
  feature = "tree-sitter-java",
  feature = "tree-sitter-javascript",
  feature = "tree-sitter-json",
  feature = "tree-sitter-kotlin",
  feature = "tree-sitter-lua",
  feature = "tree-sitter-md",
  feature = "tree-sitter-nix",
  feature = "tree-sitter-php",
  feature = "tree-sitter-python",
  feature = "tree-sitter-ruby",
  feature = "tree-sitter-rust",
  feature = "tree-sitter-scala",
  feature = "tree-sitter-solidity",
  feature = "tree-sitter-swift",
  feature = "tree-sitter-typescript",
  feature = "tree-sitter-yaml",
  feature = "tree-sitter-zig",
))]
mod parsers;
#[cfg(feature = "tree-sitter-php")]
mod php;
#[cfg(feature = "tree-sitter-python")]
mod python;
#[cfg(feature = "tree-sitter-ruby")]
mod ruby;
#[cfg(feature = "tree-sitter-rust")]
mod rust;
#[cfg(feature = "tree-sitter-scala")]
mod scala;
#[cfg(feature = "tree-sitter-solidity")]
mod solidity;
#[cfg(feature = "tree-sitter-swift")]
mod swift;
#[cfg(feature = "tree-sitter-yaml")]
mod yaml;
#[cfg(feature = "tree-sitter-zig")]
mod zig;

use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
#[cfg(feature = "tree-sitter-html")]
pub use html::Html;

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

/// this macro implements bare-bone methods for a language
#[cfg(any(
  feature = "tree-sitter-bash",
  feature = "tree-sitter-java",
  feature = "tree-sitter-javascript",
  feature = "tree-sitter-json",
  feature = "tree-sitter-lua",
  feature = "tree-sitter-md",
  feature = "tree-sitter-scala",
  feature = "tree-sitter-solidity",
  feature = "tree-sitter-typescript",
  feature = "tree-sitter-dart",
  feature = "tree-sitter-yaml",
))]
macro_rules! impl_lang {
  ($lang: ident, $func: ident) => {
    #[derive(Clone, Copy, Debug)]
    pub struct $lang;
    impl Language for $lang {
      fn kind_to_id(&self, kind: &str) -> u16 {
        self
          .get_ts_language()
          .id_for_node_kind(kind, /*named*/ true)
      }
      fn field_to_id(&self, field: &str) -> Option<u16> {
        self
          .get_ts_language()
          .field_id_for_name(field)
          .map(|f| f.get())
      }
      fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, self.clone()))
      }
    }
    impl LanguageExt for $lang {
      fn get_ts_language(&self) -> TSLanguage {
        parsers::$func().into()
      }
    }
  };
}

#[cfg(any(
  feature = "tree-sitter-c",
  feature = "tree-sitter-cpp",
  feature = "tree-sitter-c-sharp",
  feature = "tree-sitter-css",
  feature = "tree-sitter-elixir",
  feature = "tree-sitter-go",
  feature = "tree-sitter-haskell",
  feature = "tree-sitter-hcl",
  feature = "tree-sitter-kotlin",
  feature = "tree-sitter-nix",
  feature = "tree-sitter-php",
  feature = "tree-sitter-python",
  feature = "tree-sitter-ruby",
  feature = "tree-sitter-rust",
  feature = "tree-sitter-swift",
  feature = "tree-sitter-zig",
  feature = "tree-sitter-html",
))]
fn pre_process_pattern(expando: char, query: &str) -> std::borrow::Cow<'_, str> {
  let mut ret = Vec::with_capacity(query.len());
  let mut dollar_count = 0;
  for c in query.chars() {
    if c == '$' {
      dollar_count += 1;
      continue;
    }
    let need_replace = matches!(c, 'A'..='Z' | '_') // $A or $$A or $$$A
      || dollar_count == 3; // anonymous multiple
    let sigil = if need_replace { expando } else { '$' };
    ret.extend(std::iter::repeat_n(sigil, dollar_count));
    dollar_count = 0;
    ret.push(c);
  }
  // trailing anonymous multiple
  let sigil = if dollar_count == 3 { expando } else { '$' };
  ret.extend(std::iter::repeat_n(sigil, dollar_count));
  std::borrow::Cow::Owned(ret.into_iter().collect())
}

/// this macro will implement expando_char and pre_process_pattern
/// use this if your language does not accept $ as valid identifier char
#[cfg(any(
  feature = "tree-sitter-c",
  feature = "tree-sitter-cpp",
  feature = "tree-sitter-c-sharp",
  feature = "tree-sitter-css",
  feature = "tree-sitter-elixir",
  feature = "tree-sitter-go",
  feature = "tree-sitter-haskell",
  feature = "tree-sitter-hcl",
  feature = "tree-sitter-kotlin",
  feature = "tree-sitter-nix",
  feature = "tree-sitter-php",
  feature = "tree-sitter-python",
  feature = "tree-sitter-ruby",
  feature = "tree-sitter-rust",
  feature = "tree-sitter-swift",
  feature = "tree-sitter-zig",
))]
macro_rules! impl_lang_expando {
  ($lang: ident, $func: ident, $char: expr) => {
    #[derive(Clone, Copy, Debug)]
    pub struct $lang;
    impl Language for $lang {
      fn kind_to_id(&self, kind: &str) -> u16 {
        self
          .get_ts_language()
          .id_for_node_kind(kind, /*named*/ true)
      }
      fn field_to_id(&self, field: &str) -> Option<u16> {
        self
          .get_ts_language()
          .field_id_for_name(field)
          .map(|f| f.get())
      }
      fn expando_char(&self) -> char {
        $char
      }
      fn pre_process_pattern<'q>(&self, query: &'q str) -> std::borrow::Cow<'q, str> {
        pre_process_pattern(self.expando_char(), query)
      }
      fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, self.clone()))
      }
    }
    impl LanguageExt for $lang {
      fn get_ts_language(&self) -> TSLanguage {
        $crate::parsers::$func().into()
      }
    }
  };
}

pub trait Alias: Display {
  const ALIAS: &'static [&'static str];
}

/// Implements the `ALIAS` associated constant for the given lang, which is
/// then used to define the `alias` const fn and a `Deserialize` impl.
#[cfg(any(
  feature = "tree-sitter-bash",
  feature = "tree-sitter-c",
  feature = "tree-sitter-c-sharp",
  feature = "tree-sitter-cpp",
  feature = "tree-sitter-css",
  feature = "tree-sitter-dart",
  feature = "tree-sitter-elixir",
  feature = "tree-sitter-go",
  feature = "tree-sitter-haskell",
  feature = "tree-sitter-hcl",
  feature = "tree-sitter-html",
  feature = "tree-sitter-java",
  feature = "tree-sitter-javascript",
  feature = "tree-sitter-json",
  feature = "tree-sitter-kotlin",
  feature = "tree-sitter-lua",
  feature = "tree-sitter-md",
  feature = "tree-sitter-nix",
  feature = "tree-sitter-php",
  feature = "tree-sitter-python",
  feature = "tree-sitter-ruby",
  feature = "tree-sitter-rust",
  feature = "tree-sitter-scala",
  feature = "tree-sitter-solidity",
  feature = "tree-sitter-swift",
  feature = "tree-sitter-typescript",
  feature = "tree-sitter-yaml",
  feature = "tree-sitter-zig",
))]
macro_rules! impl_alias {
  ($lang:ident => $as:expr) => {
    impl Alias for $lang {
      const ALIAS: &'static [&'static str] = $as;
    }

    impl fmt::Display for $lang {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
      }
    }

    impl<'de> Deserialize<'de> for $lang {
      fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
      where
        D: Deserializer<'de>,
      {
        let vis = AliasVisitor {
          aliases: Self::ALIAS,
        };
        deserializer.deserialize_str(vis)?;
        Ok($lang)
      }
    }

    impl From<$lang> for SupportLang {
      fn from(_: $lang) -> Self {
        Self::$lang
      }
    }
  };
}
/// Generates as convenience conversions between the lang types
/// and `SupportedType`.
macro_rules! impl_aliases {
  ($( $(#[$attr:meta])* $lang:ident => $as:expr),* $(,)?) => {
    $( $(#[$attr])* impl_alias!($lang => $as);)*
    const fn alias(lang: SupportLang) -> &'static [&'static str] {
      match lang {
        $( $(#[$attr])* SupportLang::$lang => $lang::ALIAS,)*
      }
    }
  };
}

/* Customized Language with expando_char / pre_process_pattern */
// https://en.cppreference.com/w/cpp/language/identifiers
#[cfg(feature = "tree-sitter-c")]
impl_lang_expando!(C, language_c, '𐀀');
#[cfg(feature = "tree-sitter-cpp")]
impl_lang_expando!(Cpp, language_cpp, '𐀀');
// https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#643-identifiers
// all letter number is accepted
// https://www.compart.com/en/unicode/category/Nl
#[cfg(feature = "tree-sitter-c-sharp")]
impl_lang_expando!(CSharp, language_c_sharp, 'µ');
// https://www.w3.org/TR/CSS21/grammar.html#scanner
#[cfg(feature = "tree-sitter-css")]
impl_lang_expando!(Css, language_css, '_');
// https://github.com/elixir-lang/tree-sitter-elixir/blob/a2861e88a730287a60c11ea9299c033c7d076e30/grammar.js#L245
#[cfg(feature = "tree-sitter-elixir")]
impl_lang_expando!(Elixir, language_elixir, 'µ');
// we can use any Unicode code point categorized as "Letter"
// https://go.dev/ref/spec#letter
#[cfg(feature = "tree-sitter-go")]
impl_lang_expando!(Go, language_go, 'µ');
// GHC supports Unicode syntax per
// https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/unicode_syntax.html
// and the tree-sitter-haskell grammar parses it too.
#[cfg(feature = "tree-sitter-haskell")]
impl_lang_expando!(Haskell, language_haskell, 'µ');
// https://developer.hashicorp.com/terraform/language/syntax/configuration#identifiers
#[cfg(feature = "tree-sitter-hcl")]
impl_lang_expando!(Hcl, language_hcl, 'µ');
// https://github.com/fwcd/tree-sitter-kotlin/pull/93
#[cfg(feature = "tree-sitter-kotlin")]
impl_lang_expando!(Kotlin, language_kotlin, 'µ');
// Nix uses $ for string interpolation (e.g., "${pkgs.hello}")
#[cfg(feature = "tree-sitter-nix")]
impl_lang_expando!(Nix, language_nix, '_');
// PHP accepts unicode to be used as some name not var name though
#[cfg(feature = "tree-sitter-php")]
impl_lang_expando!(Php, language_php, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://docs.python.org/3/reference/lexical_analysis.html#identifiers
// see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
#[cfg(feature = "tree-sitter-python")]
impl_lang_expando!(Python, language_python, 'µ');
// https://github.com/tree-sitter/tree-sitter-ruby/blob/f257f3f57833d584050336921773738a3fd8ca22/grammar.js#L30C26-L30C78
#[cfg(feature = "tree-sitter-ruby")]
impl_lang_expando!(Ruby, language_ruby, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://doc.rust-lang.org/reference/identifiers.html
#[cfg(feature = "tree-sitter-rust")]
impl_lang_expando!(Rust, language_rust, 'µ');
//https://docs.swift.org/swift-book/documentation/the-swift-programming-language/lexicalstructure/#Identifiers
#[cfg(feature = "tree-sitter-swift")]
impl_lang_expando!(Swift, language_swift, 'µ');
// Zig identifiers are [A-Za-z_][A-Za-z0-9_]*; '$' is not one of them.
// https://ziglang.org/documentation/master/#Identifiers
#[cfg(feature = "tree-sitter-zig")]
impl_lang_expando!(Zig, language_zig, '_');

// Stub Language without preprocessing
// Language Name, tree-sitter-name, alias, extension
#[cfg(feature = "tree-sitter-bash")]
impl_lang!(Bash, language_bash);
#[cfg(feature = "tree-sitter-java")]
impl_lang!(Java, language_java);
#[cfg(feature = "tree-sitter-javascript")]
impl_lang!(JavaScript, language_javascript);
#[cfg(feature = "tree-sitter-json")]
impl_lang!(Json, language_json);
#[cfg(feature = "tree-sitter-lua")]
impl_lang!(Lua, language_lua);
#[cfg(feature = "tree-sitter-md")]
impl_lang!(Markdown, language_markdown);
#[cfg(feature = "tree-sitter-scala")]
impl_lang!(Scala, language_scala);
#[cfg(feature = "tree-sitter-solidity")]
impl_lang!(Solidity, language_solidity);
#[cfg(feature = "tree-sitter-typescript")]
impl_lang!(Tsx, language_tsx);
#[cfg(feature = "tree-sitter-typescript")]
impl_lang!(TypeScript, language_typescript);
#[cfg(feature = "tree-sitter-dart")]
impl_lang!(Dart, language_dart);
#[cfg(feature = "tree-sitter-yaml")]
impl_lang!(Yaml, language_yaml);
// See ripgrep for extensions
// https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/default_types.rs

/// Represents all built-in languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
pub enum SupportLang {
  #[cfg(feature = "tree-sitter-bash")]
  Bash,
  #[cfg(feature = "tree-sitter-c")]
  C,
  #[cfg(feature = "tree-sitter-cpp")]
  Cpp,
  #[cfg(feature = "tree-sitter-c-sharp")]
  CSharp,
  #[cfg(feature = "tree-sitter-css")]
  Css,
  #[cfg(feature = "tree-sitter-dart")]
  Dart,
  #[cfg(feature = "tree-sitter-go")]
  Go,
  #[cfg(feature = "tree-sitter-elixir")]
  Elixir,
  #[cfg(feature = "tree-sitter-haskell")]
  Haskell,
  #[cfg(feature = "tree-sitter-hcl")]
  Hcl,
  #[cfg(feature = "tree-sitter-html")]
  Html,
  #[cfg(feature = "tree-sitter-java")]
  Java,
  #[cfg(feature = "tree-sitter-javascript")]
  JavaScript,
  #[cfg(feature = "tree-sitter-json")]
  Json,
  #[cfg(feature = "tree-sitter-kotlin")]
  Kotlin,
  #[cfg(feature = "tree-sitter-lua")]
  Lua,
  #[cfg(feature = "tree-sitter-md")]
  Markdown,
  #[cfg(feature = "tree-sitter-nix")]
  Nix,
  #[cfg(feature = "tree-sitter-php")]
  Php,
  #[cfg(feature = "tree-sitter-python")]
  Python,
  #[cfg(feature = "tree-sitter-ruby")]
  Ruby,
  #[cfg(feature = "tree-sitter-rust")]
  Rust,
  #[cfg(feature = "tree-sitter-scala")]
  Scala,
  #[cfg(feature = "tree-sitter-solidity")]
  Solidity,
  #[cfg(feature = "tree-sitter-swift")]
  Swift,
  #[cfg(feature = "tree-sitter-typescript")]
  Tsx,
  #[cfg(feature = "tree-sitter-typescript")]
  TypeScript,
  #[cfg(feature = "tree-sitter-yaml")]
  Yaml,
  #[cfg(feature = "tree-sitter-zig")]
  Zig,
}

impl SupportLang {
  pub const fn all_langs() -> &'static [SupportLang] {
    &[
      #[cfg(feature = "tree-sitter-bash")]
      SupportLang::Bash,
      #[cfg(feature = "tree-sitter-c")]
      SupportLang::C,
      #[cfg(feature = "tree-sitter-cpp")]
      SupportLang::Cpp,
      #[cfg(feature = "tree-sitter-c-sharp")]
      SupportLang::CSharp,
      #[cfg(feature = "tree-sitter-css")]
      SupportLang::Css,
      #[cfg(feature = "tree-sitter-dart")]
      SupportLang::Dart,
      #[cfg(feature = "tree-sitter-elixir")]
      SupportLang::Elixir,
      #[cfg(feature = "tree-sitter-go")]
      SupportLang::Go,
      #[cfg(feature = "tree-sitter-haskell")]
      SupportLang::Haskell,
      #[cfg(feature = "tree-sitter-hcl")]
      SupportLang::Hcl,
      #[cfg(feature = "tree-sitter-html")]
      SupportLang::Html,
      #[cfg(feature = "tree-sitter-java")]
      SupportLang::Java,
      #[cfg(feature = "tree-sitter-javascript")]
      SupportLang::JavaScript,
      #[cfg(feature = "tree-sitter-json")]
      SupportLang::Json,
      #[cfg(feature = "tree-sitter-kotlin")]
      SupportLang::Kotlin,
      #[cfg(feature = "tree-sitter-lua")]
      SupportLang::Lua,
      #[cfg(feature = "tree-sitter-md")]
      SupportLang::Markdown,
      #[cfg(feature = "tree-sitter-nix")]
      SupportLang::Nix,
      #[cfg(feature = "tree-sitter-php")]
      SupportLang::Php,
      #[cfg(feature = "tree-sitter-python")]
      SupportLang::Python,
      #[cfg(feature = "tree-sitter-ruby")]
      SupportLang::Ruby,
      #[cfg(feature = "tree-sitter-rust")]
      SupportLang::Rust,
      #[cfg(feature = "tree-sitter-scala")]
      SupportLang::Scala,
      #[cfg(feature = "tree-sitter-solidity")]
      SupportLang::Solidity,
      #[cfg(feature = "tree-sitter-swift")]
      SupportLang::Swift,
      #[cfg(feature = "tree-sitter-typescript")]
      SupportLang::Tsx,
      #[cfg(feature = "tree-sitter-typescript")]
      SupportLang::TypeScript,
      #[cfg(feature = "tree-sitter-yaml")]
      SupportLang::Yaml,
      #[cfg(feature = "tree-sitter-zig")]
      SupportLang::Zig,
    ]
  }

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
#[cfg(any(
  feature = "tree-sitter-bash",
  feature = "tree-sitter-c",
  feature = "tree-sitter-c-sharp",
  feature = "tree-sitter-cpp",
  feature = "tree-sitter-css",
  feature = "tree-sitter-dart",
  feature = "tree-sitter-elixir",
  feature = "tree-sitter-go",
  feature = "tree-sitter-haskell",
  feature = "tree-sitter-hcl",
  feature = "tree-sitter-html",
  feature = "tree-sitter-java",
  feature = "tree-sitter-javascript",
  feature = "tree-sitter-json",
  feature = "tree-sitter-kotlin",
  feature = "tree-sitter-lua",
  feature = "tree-sitter-md",
  feature = "tree-sitter-nix",
  feature = "tree-sitter-php",
  feature = "tree-sitter-python",
  feature = "tree-sitter-ruby",
  feature = "tree-sitter-rust",
  feature = "tree-sitter-scala",
  feature = "tree-sitter-solidity",
  feature = "tree-sitter-swift",
  feature = "tree-sitter-typescript",
  feature = "tree-sitter-yaml",
  feature = "tree-sitter-zig",
))]
struct AliasVisitor {
  aliases: &'static [&'static str],
}

#[cfg(any(
  feature = "tree-sitter-bash",
  feature = "tree-sitter-c",
  feature = "tree-sitter-c-sharp",
  feature = "tree-sitter-cpp",
  feature = "tree-sitter-css",
  feature = "tree-sitter-dart",
  feature = "tree-sitter-elixir",
  feature = "tree-sitter-go",
  feature = "tree-sitter-haskell",
  feature = "tree-sitter-hcl",
  feature = "tree-sitter-html",
  feature = "tree-sitter-java",
  feature = "tree-sitter-javascript",
  feature = "tree-sitter-json",
  feature = "tree-sitter-kotlin",
  feature = "tree-sitter-lua",
  feature = "tree-sitter-md",
  feature = "tree-sitter-nix",
  feature = "tree-sitter-php",
  feature = "tree-sitter-python",
  feature = "tree-sitter-ruby",
  feature = "tree-sitter-rust",
  feature = "tree-sitter-scala",
  feature = "tree-sitter-solidity",
  feature = "tree-sitter-swift",
  feature = "tree-sitter-typescript",
  feature = "tree-sitter-yaml",
  feature = "tree-sitter-zig",
))]
impl Visitor<'_> for AliasVisitor {
  type Value = &'static str;

  fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "one of {:?}", self.aliases)
  }

  fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    self
      .aliases
      .iter()
      .copied()
      .find(|&a| v.eq_ignore_ascii_case(a))
      .ok_or_else(|| de::Error::invalid_value(de::Unexpected::Str(v), &self))
  }
}

impl_aliases! {
  #[cfg(feature = "tree-sitter-bash")]
  Bash => &["bash"],
  #[cfg(feature = "tree-sitter-c")]
  C => &["c"],
  #[cfg(feature = "tree-sitter-cpp")]
  Cpp => &["cc", "c++", "cpp", "cxx"],
  #[cfg(feature = "tree-sitter-c-sharp")]
  CSharp => &["cs", "csharp"],
  #[cfg(feature = "tree-sitter-css")]
  Css => &["css"],
  #[cfg(feature = "tree-sitter-dart")]
  Dart => &["dart"],
  #[cfg(feature = "tree-sitter-elixir")]
  Elixir => &["ex", "elixir"],
  #[cfg(feature = "tree-sitter-go")]
  Go => &["go", "golang"],
  #[cfg(feature = "tree-sitter-haskell")]
  Haskell => &["hs", "haskell"],
  #[cfg(feature = "tree-sitter-hcl")]
  Hcl => &["hcl"],
  #[cfg(feature = "tree-sitter-html")]
  Html => &["html"],
  #[cfg(feature = "tree-sitter-java")]
  Java => &["java"],
  #[cfg(feature = "tree-sitter-javascript")]
  JavaScript => &["javascript", "js", "jsx"],
  #[cfg(feature = "tree-sitter-json")]
  Json => &["json"],
  #[cfg(feature = "tree-sitter-kotlin")]
  Kotlin => &["kotlin", "kt"],
  #[cfg(feature = "tree-sitter-lua")]
  Lua => &["lua"],
  #[cfg(feature = "tree-sitter-md")]
  Markdown => &["markdown", "md"],
  #[cfg(feature = "tree-sitter-nix")]
  Nix => &["nix"],
  #[cfg(feature = "tree-sitter-php")]
  Php => &["php"],
  #[cfg(feature = "tree-sitter-python")]
  Python => &["py", "python"],
  #[cfg(feature = "tree-sitter-ruby")]
  Ruby => &["rb", "ruby"],
  #[cfg(feature = "tree-sitter-rust")]
  Rust => &["rs", "rust"],
  #[cfg(feature = "tree-sitter-scala")]
  Scala => &["scala"],
  #[cfg(feature = "tree-sitter-solidity")]
  Solidity => &["sol", "solidity"],
  #[cfg(feature = "tree-sitter-swift")]
  Swift => &["swift"],
  #[cfg(feature = "tree-sitter-typescript")]
  TypeScript => &["ts", "typescript"],
  #[cfg(feature = "tree-sitter-typescript")]
  Tsx => &["tsx"],
  #[cfg(feature = "tree-sitter-yaml")]
  Yaml => &["yaml", "yml"],
  #[cfg(feature = "tree-sitter-zig")]
  Zig => &["zig"],
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

macro_rules! execute_lang_method {
  ($me: path, $method: ident, $($pname:tt),*) => {
    match *$me {
      #[cfg(feature = "tree-sitter-bash")]
      SupportLang::Bash => Bash.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-c")]
      SupportLang::C => C.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-cpp")]
      SupportLang::Cpp => Cpp.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-c-sharp")]
      SupportLang::CSharp => CSharp.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-css")]
      SupportLang::Css => Css.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-dart")]
      SupportLang::Dart => Dart.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-elixir")]
      SupportLang::Elixir => Elixir.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-go")]
      SupportLang::Go => Go.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-haskell")]
      SupportLang::Haskell => Haskell.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-hcl")]
      SupportLang::Hcl => Hcl.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-html")]
      SupportLang::Html => Html.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-java")]
      SupportLang::Java => Java.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-javascript")]
      SupportLang::JavaScript => JavaScript.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-json")]
      SupportLang::Json => Json.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-kotlin")]
      SupportLang::Kotlin => Kotlin.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-lua")]
      SupportLang::Lua => Lua.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-md")]
      SupportLang::Markdown => Markdown.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-nix")]
      SupportLang::Nix => Nix.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-php")]
      SupportLang::Php => Php.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-python")]
      SupportLang::Python => Python.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-ruby")]
      SupportLang::Ruby => Ruby.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-rust")]
      SupportLang::Rust => Rust.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-scala")]
      SupportLang::Scala => Scala.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-solidity")]
      SupportLang::Solidity => Solidity.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-swift")]
      SupportLang::Swift => Swift.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-typescript")]
      SupportLang::Tsx => Tsx.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-typescript")]
      SupportLang::TypeScript => TypeScript.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-yaml")]
      SupportLang::Yaml => Yaml.$method($($pname,)*),
      #[cfg(feature = "tree-sitter-zig")]
      SupportLang::Zig => Zig.$method($($pname,)*),
    }
  }
}

macro_rules! impl_lang_method {
  ($method: ident, ($($pname:tt: $ptype:ty),*) => $return_type: ty) => {
    #[inline]
    fn $method(&self, $($pname: $ptype),*) -> $return_type {
      execute_lang_method!{ self, $method, $($pname),* }
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
    execute_lang_method! { self, pre_process_pattern, _query }
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
    match self {
      #[cfg(feature = "tree-sitter-html")]
      SupportLang::Html => Html.extract_injections(_root),
      _ => Vec::new(),
    }
  }
}

fn extensions(lang: SupportLang) -> &'static [&'static str] {
  match lang {
    #[cfg(feature = "tree-sitter-bash")]
    SupportLang::Bash => &[
      "bash", "bats", "cgi", "command", "env", "fcgi", "ksh", "sh", "tmux", "tool", "zsh",
    ],
    #[cfg(feature = "tree-sitter-c")]
    SupportLang::C => &["c", "h"],
    #[cfg(feature = "tree-sitter-cpp")]
    SupportLang::Cpp => &["cc", "hpp", "cpp", "c++", "hh", "cxx", "cu", "ino"],
    #[cfg(feature = "tree-sitter-c-sharp")]
    SupportLang::CSharp => &["cs"],
    #[cfg(feature = "tree-sitter-css")]
    SupportLang::Css => &["css", "scss"],
    #[cfg(feature = "tree-sitter-dart")]
    SupportLang::Dart => &["dart"],
    #[cfg(feature = "tree-sitter-elixir")]
    SupportLang::Elixir => &["ex", "exs"],
    #[cfg(feature = "tree-sitter-go")]
    SupportLang::Go => &["go"],
    #[cfg(feature = "tree-sitter-haskell")]
    SupportLang::Haskell => &["hs"],
    #[cfg(feature = "tree-sitter-hcl")]
    SupportLang::Hcl => &["hcl", "nomad", "tf", "tfvars", "workflow"],
    #[cfg(feature = "tree-sitter-html")]
    SupportLang::Html => &["html", "htm", "xhtml"],
    #[cfg(feature = "tree-sitter-java")]
    SupportLang::Java => &["java"],
    #[cfg(feature = "tree-sitter-javascript")]
    SupportLang::JavaScript => &["cjs", "js", "mjs", "jsx"],
    #[cfg(feature = "tree-sitter-json")]
    SupportLang::Json => &["json"],
    #[cfg(feature = "tree-sitter-kotlin")]
    SupportLang::Kotlin => &["kt", "ktm", "kts"],
    #[cfg(feature = "tree-sitter-lua")]
    SupportLang::Lua => &["lua"],
    #[cfg(feature = "tree-sitter-md")]
    SupportLang::Markdown => &["markdown", "md"],
    #[cfg(feature = "tree-sitter-nix")]
    SupportLang::Nix => &["nix"],
    #[cfg(feature = "tree-sitter-php")]
    SupportLang::Php => &["php"],
    #[cfg(feature = "tree-sitter-python")]
    SupportLang::Python => &["py", "py3", "pyi", "bzl", "bazel"],
    #[cfg(feature = "tree-sitter-ruby")]
    SupportLang::Ruby => &["rb", "rbw", "gemspec"],
    #[cfg(feature = "tree-sitter-rust")]
    SupportLang::Rust => &["rs"],
    #[cfg(feature = "tree-sitter-scala")]
    SupportLang::Scala => &["scala", "sc", "sbt"],
    #[cfg(feature = "tree-sitter-solidity")]
    SupportLang::Solidity => &["sol"],
    #[cfg(feature = "tree-sitter-swift")]
    SupportLang::Swift => &["swift"],
    #[cfg(feature = "tree-sitter-typescript")]
    SupportLang::TypeScript => &["ts", "cts", "mts"],
    #[cfg(feature = "tree-sitter-typescript")]
    SupportLang::Tsx => &["tsx"],
    #[cfg(feature = "tree-sitter-yaml")]
    SupportLang::Yaml => &["yaml", "yml"],
    #[cfg(feature = "tree-sitter-zig")]
    SupportLang::Zig => &["zig"],
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

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::{Pattern, matcher::MatcherExt};

  pub fn test_match_lang(query: &str, source: &str, lang: impl LanguageExt) {
    let cand = lang.ast_grep(source);
    let pattern = Pattern::new(query, lang);
    assert!(
      pattern.find_node(cand.root()).is_some(),
      "goal: {pattern:?}, candidate: {}",
      cand.root().get_inner_node().to_sexp(),
    );
  }

  pub fn test_non_match_lang(query: &str, source: &str, lang: impl LanguageExt) {
    let cand = lang.ast_grep(source);
    let pattern = Pattern::new(query, lang);
    assert!(
      pattern.find_node(cand.root()).is_none(),
      "goal: {pattern:?}, candidate: {}",
      cand.root().get_inner_node().to_sexp(),
    );
  }

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
  fn test_guess_by_extension() {
    let path = Path::new("foo.rs");
    assert_eq!(from_extension(path), Some(SupportLang::Rust));
  }

  #[cfg(feature = "tree-sitter-md")]
  #[test]
  fn test_markdown_extensions() {
    let path = Path::new("README.md");
    assert_eq!(from_extension(path), Some(SupportLang::Markdown));
    let path = Path::new("README.markdown");
    assert_eq!(from_extension(path), Some(SupportLang::Markdown));
  }

  #[cfg(feature = "tree-sitter-zig")]
  #[test]
  fn test_zig_extension_and_alias() {
    let path = Path::new("build.zig");
    assert_eq!(from_extension(path), Some(SupportLang::Zig));
    assert_eq!("zig".parse::<SupportLang>().unwrap(), SupportLang::Zig);
  }

  #[cfg(not(feature = "tree-sitter-kotlin"))]
  #[test]
  fn test_compiled_out_language_is_not_registered() {
    for name in ["kotlin", "kt"] {
      match name.parse::<SupportLang>() {
        Err(SupportLangErr::LanguageNotSupported(rejected)) => assert_eq!(rejected, name),
        Ok(lang) => panic!("compiled-out language resolved to {lang}"),
      }
    }
    for extension in ["kt", "ktm", "kts"] {
      assert_eq!(
        from_extension(Path::new(&format!("example.{extension}"))),
        None
      );
    }
  }

  #[cfg(feature = "builtin-parser")]
  #[test]
  fn test_default_registry_is_unchanged() {
    let names: Vec<_> = SupportLang::all_langs()
      .iter()
      .map(ToString::to_string)
      .collect();
    assert_eq!(
      names,
      [
        "Bash",
        "C",
        "Cpp",
        "CSharp",
        "Css",
        "Dart",
        "Elixir",
        "Go",
        "Haskell",
        "Hcl",
        "Html",
        "Java",
        "JavaScript",
        "Json",
        "Kotlin",
        "Lua",
        "Markdown",
        "Nix",
        "Php",
        "Python",
        "Ruby",
        "Rust",
        "Scala",
        "Solidity",
        "Swift",
        "Tsx",
        "TypeScript",
        "Yaml",
        "Zig"
      ]
    );
    for &lang in SupportLang::all_langs() {
      for name in alias(lang) {
        assert_eq!(name.parse::<SupportLang>().unwrap(), lang);
      }
      let types = lang.file_types();
      for extension in extensions(lang) {
        let path = format!("example.{extension}");
        assert!(types.matched(Path::new(&path), false).is_whitelist());
      }
    }
  }
}
