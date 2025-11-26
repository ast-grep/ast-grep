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

#[cfg(feature = "lang-bash")]
mod bash;
#[cfg(feature = "lang-cpp")]
mod cpp;
#[cfg(feature = "lang-csharp")]
mod csharp;
#[cfg(feature = "lang-css")]
mod css;
#[cfg(feature = "lang-elixir")]
mod elixir;
#[cfg(feature = "lang-go")]
mod go;
#[cfg(feature = "lang-haskell")]
mod haskell;
#[cfg(feature = "lang-hcl")]
mod hcl;
#[cfg(feature = "lang-html")]
mod html;
#[cfg(feature = "lang-json")]
mod json;
#[cfg(feature = "lang-kotlin")]
mod kotlin;
#[cfg(feature = "lang-lua")]
mod lua;
#[cfg(feature = "lang-nix")]
mod nix;
mod parsers;
#[cfg(feature = "lang-php")]
mod php;
#[cfg(feature = "lang-python")]
mod python;
#[cfg(feature = "lang-ruby")]
mod ruby;
#[cfg(feature = "lang-rust")]
mod rust;
#[cfg(feature = "lang-scala")]
mod scala;
#[cfg(feature = "lang-solidity")]
mod solidity;
#[cfg(feature = "lang-swift")]
mod swift;
#[cfg(feature = "lang-yaml")]
mod yaml;

use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
#[cfg(feature = "lang-html")]
pub use html::Html;

use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::tree_sitter::{StrDoc, TSLanguage, TSRange};
use ast_grep_core::Node;
use ignore::types::{Types, TypesBuilder};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::iter::repeat;
use std::path::Path;
use std::str::FromStr;

pub use ast_grep_core::language::Language;
pub use ast_grep_core::tree_sitter::LanguageExt;

/// this macro implements bare-bone methods for a language
#[allow(unused_macros)]
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

#[allow(dead_code)]
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
    ret.extend(repeat(sigil).take(dollar_count));
    dollar_count = 0;
    ret.push(c);
  }
  // trailing anonymous multiple
  let sigil = if dollar_count == 3 { expando } else { '$' };
  ret.extend(repeat(sigil).take(dollar_count));
  std::borrow::Cow::Owned(ret.into_iter().collect())
}

/// this macro will implement expando_char and pre_process_pattern
/// use this if your language does not accept $ as valid identifier char
#[allow(unused_macros)]
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
#[allow(unused_macros)]
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
/* Customized Language with expando_char / pre_process_pattern */
// https://en.cppreference.com/w/cpp/language/identifiers
#[cfg(feature = "lang-c")]
impl_lang_expando!(C, language_c, '𐀀');
#[cfg(feature = "lang-cpp")]
impl_lang_expando!(Cpp, language_cpp, '𐀀');
// https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#643-identifiers
// all letter number is accepted
// https://www.compart.com/en/unicode/category/Nl
#[cfg(feature = "lang-csharp")]
impl_lang_expando!(CSharp, language_c_sharp, 'µ');
// https://www.w3.org/TR/CSS21/grammar.html#scanner
#[cfg(feature = "lang-css")]
impl_lang_expando!(Css, language_css, '_');
// https://github.com/elixir-lang/tree-sitter-elixir/blob/a2861e88a730287a60c11ea9299c033c7d076e30/grammar.js#L245
#[cfg(feature = "lang-elixir")]
impl_lang_expando!(Elixir, language_elixir, 'µ');
// we can use any Unicode code point categorized as "Letter"
// https://go.dev/ref/spec#letter
#[cfg(feature = "lang-go")]
impl_lang_expando!(Go, language_go, 'µ');
// GHC supports Unicode syntax per
// https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/unicode_syntax.html
// and the tree-sitter-haskell grammar parses it too.
#[cfg(feature = "lang-haskell")]
impl_lang_expando!(Haskell, language_haskell, 'µ');
// https://developer.hashicorp.com/terraform/language/syntax/configuration#identifiers
#[cfg(feature = "lang-hcl")]
impl_lang_expando!(Hcl, language_hcl, 'µ');
// https://github.com/fwcd/tree-sitter-kotlin/pull/93
#[cfg(feature = "lang-kotlin")]
impl_lang_expando!(Kotlin, language_kotlin, 'µ');
// Nix uses $ for string interpolation (e.g., "${pkgs.hello}")
#[cfg(feature = "lang-nix")]
impl_lang_expando!(Nix, language_nix, '_');
// PHP accepts unicode to be used as some name not var name though
#[cfg(feature = "lang-php")]
impl_lang_expando!(Php, language_php, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://docs.python.org/3/reference/lexical_analysis.html#identifiers
// see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
#[cfg(feature = "lang-python")]
impl_lang_expando!(Python, language_python, 'µ');
// https://github.com/tree-sitter/tree-sitter-ruby/blob/f257f3f57833d584050336921773738a3fd8ca22/grammar.js#L30C26-L30C78
#[cfg(feature = "lang-ruby")]
impl_lang_expando!(Ruby, language_ruby, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://doc.rust-lang.org/reference/identifiers.html
#[cfg(feature = "lang-rust")]
impl_lang_expando!(Rust, language_rust, 'µ');
//https://docs.swift.org/swift-book/documentation/the-swift-programming-language/lexicalstructure/#Identifiers
#[cfg(feature = "lang-swift")]
impl_lang_expando!(Swift, language_swift, 'µ');

// Stub Language without preprocessing
// Language Name, tree-sitter-name, alias, extension
#[cfg(feature = "lang-bash")]
impl_lang!(Bash, language_bash);
#[cfg(feature = "lang-java")]
impl_lang!(Java, language_java);
#[cfg(feature = "lang-javascript")]
impl_lang!(JavaScript, language_javascript);
#[cfg(feature = "lang-json")]
impl_lang!(Json, language_json);
#[cfg(feature = "lang-lua")]
impl_lang!(Lua, language_lua);
#[cfg(feature = "lang-scala")]
impl_lang!(Scala, language_scala);
#[cfg(feature = "lang-solidity")]
impl_lang!(Solidity, language_solidity);
#[cfg(feature = "lang-typescript")]
impl_lang!(Tsx, language_tsx);
#[cfg(feature = "lang-typescript")]
impl_lang!(TypeScript, language_typescript);
#[cfg(feature = "lang-yaml")]
impl_lang!(Yaml, language_yaml);
// See ripgrep for extensions
// https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/default_types.rs

/// Represents all built-in languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
#[non_exhaustive]
pub enum SupportLang {
  #[cfg(feature = "lang-bash")]
  Bash,
  #[cfg(feature = "lang-c")]
  C,
  #[cfg(feature = "lang-cpp")]
  Cpp,
  #[cfg(feature = "lang-csharp")]
  CSharp,
  #[cfg(feature = "lang-css")]
  Css,
  #[cfg(feature = "lang-go")]
  Go,
  #[cfg(feature = "lang-elixir")]
  Elixir,
  #[cfg(feature = "lang-haskell")]
  Haskell,
  #[cfg(feature = "lang-hcl")]
  Hcl,
  #[cfg(feature = "lang-html")]
  Html,
  #[cfg(feature = "lang-java")]
  Java,
  #[cfg(feature = "lang-javascript")]
  JavaScript,
  #[cfg(feature = "lang-json")]
  Json,
  #[cfg(feature = "lang-kotlin")]
  Kotlin,
  #[cfg(feature = "lang-lua")]
  Lua,
  #[cfg(feature = "lang-nix")]
  Nix,
  #[cfg(feature = "lang-php")]
  Php,
  #[cfg(feature = "lang-python")]
  Python,
  #[cfg(feature = "lang-ruby")]
  Ruby,
  #[cfg(feature = "lang-rust")]
  Rust,
  #[cfg(feature = "lang-scala")]
  Scala,
  #[cfg(feature = "lang-solidity")]
  Solidity,
  #[cfg(feature = "lang-swift")]
  Swift,
  #[cfg(feature = "lang-typescript")]
  Tsx,
  #[cfg(feature = "lang-typescript")]
  TypeScript,
  #[cfg(feature = "lang-yaml")]
  Yaml,
}

impl SupportLang {
  pub fn all_langs() -> &'static [SupportLang] {
    #[allow(unused)]
    use SupportLang::*;
    &[
      #[cfg(feature = "lang-bash")]
      Bash,
      #[cfg(feature = "lang-c")]
      C,
      #[cfg(feature = "lang-cpp")]
      Cpp,
      #[cfg(feature = "lang-csharp")]
      CSharp,
      #[cfg(feature = "lang-css")]
      Css,
      #[cfg(feature = "lang-elixir")]
      Elixir,
      #[cfg(feature = "lang-go")]
      Go,
      #[cfg(feature = "lang-haskell")]
      Haskell,
      #[cfg(feature = "lang-hcl")]
      Hcl,
      #[cfg(feature = "lang-html")]
      Html,
      #[cfg(feature = "lang-java")]
      Java,
      #[cfg(feature = "lang-javascript")]
      JavaScript,
      #[cfg(feature = "lang-json")]
      Json,
      #[cfg(feature = "lang-kotlin")]
      Kotlin,
      #[cfg(feature = "lang-lua")]
      Lua,
      #[cfg(feature = "lang-nix")]
      Nix,
      #[cfg(feature = "lang-php")]
      Php,
      #[cfg(feature = "lang-python")]
      Python,
      #[cfg(feature = "lang-ruby")]
      Ruby,
      #[cfg(feature = "lang-rust")]
      Rust,
      #[cfg(feature = "lang-scala")]
      Scala,
      #[cfg(feature = "lang-solidity")]
      Solidity,
      #[cfg(feature = "lang-swift")]
      Swift,
      #[cfg(feature = "lang-typescript")]
      Tsx,
      #[cfg(feature = "lang-typescript")]
      TypeScript,
      #[cfg(feature = "lang-yaml")]
      Yaml,
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
struct AliasVisitor {
  aliases: &'static [&'static str],
}

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

#[cfg(feature = "lang-bash")]
impl_alias!(Bash => &["bash"]);
#[cfg(feature = "lang-c")]
impl_alias!(C => &["c"]);
#[cfg(feature = "lang-cpp")]
impl_alias!(Cpp => &["cc", "c++", "cpp", "cxx"]);
#[cfg(feature = "lang-csharp")]
impl_alias!(CSharp => &["cs", "csharp"]);
#[cfg(feature = "lang-css")]
impl_alias!(Css => &["css"]);
#[cfg(feature = "lang-elixir")]
impl_alias!(Elixir => &["ex", "elixir"]);
#[cfg(feature = "lang-go")]
impl_alias!(Go => &["go", "golang"]);
#[cfg(feature = "lang-haskell")]
impl_alias!(Haskell => &["hs", "haskell"]);
#[cfg(feature = "lang-hcl")]
impl_alias!(Hcl => &["hcl"]);
#[cfg(feature = "lang-html")]
impl_alias!(Html => &["html"]);
#[cfg(feature = "lang-java")]
impl_alias!(Java => &["java"]);
#[cfg(feature = "lang-javascript")]
impl_alias!(JavaScript => &["javascript", "js", "jsx"]);
#[cfg(feature = "lang-json")]
impl_alias!(Json => &["json"]);
#[cfg(feature = "lang-kotlin")]
impl_alias!(Kotlin => &["kotlin", "kt"]);
#[cfg(feature = "lang-lua")]
impl_alias!(Lua => &["lua"]);
#[cfg(feature = "lang-nix")]
impl_alias!(Nix => &["nix"]);
#[cfg(feature = "lang-php")]
impl_alias!(Php => &["php"]);
#[cfg(feature = "lang-python")]
impl_alias!(Python => &["py", "python"]);
#[cfg(feature = "lang-ruby")]
impl_alias!(Ruby => &["rb", "ruby"]);
#[cfg(feature = "lang-rust")]
impl_alias!(Rust => &["rs", "rust"]);
#[cfg(feature = "lang-scala")]
impl_alias!(Scala => &["scala"]);
#[cfg(feature = "lang-solidity")]
impl_alias!(Solidity => &["sol", "solidity"]);
#[cfg(feature = "lang-swift")]
impl_alias!(Swift => &["swift"]);
#[cfg(feature = "lang-typescript")]
impl_alias!(TypeScript => &["ts", "typescript"]);
#[cfg(feature = "lang-typescript")]
impl_alias!(Tsx => &["tsx"]);
#[cfg(feature = "lang-yaml")]
impl_alias!(Yaml => &["yaml", "yml"]);

fn alias(lang: SupportLang) -> &'static [&'static str] {
  match lang {
    #[cfg(feature = "lang-bash")]
    SupportLang::Bash => <Bash as Alias>::ALIAS,
    #[cfg(feature = "lang-c")]
    SupportLang::C => <C as Alias>::ALIAS,
    #[cfg(feature = "lang-cpp")]
    SupportLang::Cpp => <Cpp as Alias>::ALIAS,
    #[cfg(feature = "lang-csharp")]
    SupportLang::CSharp => <CSharp as Alias>::ALIAS,
    #[cfg(feature = "lang-css")]
    SupportLang::Css => <Css as Alias>::ALIAS,
    #[cfg(feature = "lang-elixir")]
    SupportLang::Elixir => <Elixir as Alias>::ALIAS,
    #[cfg(feature = "lang-go")]
    SupportLang::Go => <Go as Alias>::ALIAS,
    #[cfg(feature = "lang-haskell")]
    SupportLang::Haskell => <Haskell as Alias>::ALIAS,
    #[cfg(feature = "lang-hcl")]
    SupportLang::Hcl => <Hcl as Alias>::ALIAS,
    #[cfg(feature = "lang-html")]
    SupportLang::Html => <Html as Alias>::ALIAS,
    #[cfg(feature = "lang-java")]
    SupportLang::Java => <Java as Alias>::ALIAS,
    #[cfg(feature = "lang-javascript")]
    SupportLang::JavaScript => <JavaScript as Alias>::ALIAS,
    #[cfg(feature = "lang-json")]
    SupportLang::Json => <Json as Alias>::ALIAS,
    #[cfg(feature = "lang-kotlin")]
    SupportLang::Kotlin => <Kotlin as Alias>::ALIAS,
    #[cfg(feature = "lang-lua")]
    SupportLang::Lua => <Lua as Alias>::ALIAS,
    #[cfg(feature = "lang-nix")]
    SupportLang::Nix => <Nix as Alias>::ALIAS,
    #[cfg(feature = "lang-php")]
    SupportLang::Php => <Php as Alias>::ALIAS,
    #[cfg(feature = "lang-python")]
    SupportLang::Python => <Python as Alias>::ALIAS,
    #[cfg(feature = "lang-ruby")]
    SupportLang::Ruby => <Ruby as Alias>::ALIAS,
    #[cfg(feature = "lang-rust")]
    SupportLang::Rust => <Rust as Alias>::ALIAS,
    #[cfg(feature = "lang-scala")]
    SupportLang::Scala => <Scala as Alias>::ALIAS,
    #[cfg(feature = "lang-solidity")]
    SupportLang::Solidity => <Solidity as Alias>::ALIAS,
    #[cfg(feature = "lang-swift")]
    SupportLang::Swift => <Swift as Alias>::ALIAS,
    #[cfg(feature = "lang-typescript")]
    SupportLang::Tsx => <Tsx as Alias>::ALIAS,
    #[cfg(feature = "lang-typescript")]
    SupportLang::TypeScript => <TypeScript as Alias>::ALIAS,
    #[cfg(feature = "lang-yaml")]
    SupportLang::Yaml => <Yaml as Alias>::ALIAS,
    #[allow(unreachable_patterns)]
    _ => unreachable!("No language features enabled"),
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

macro_rules! execute_lang_method {
  ($me: path, $method: ident, $($pname:tt),*) => {
    #[allow(unused)]
    use SupportLang as S;
    match $me {
      #[cfg(feature = "lang-bash")]
      S::Bash => Bash.$method($($pname,)*),
      #[cfg(feature = "lang-c")]
      S::C => C.$method($($pname,)*),
      #[cfg(feature = "lang-cpp")]
      S::Cpp => Cpp.$method($($pname,)*),
      #[cfg(feature = "lang-csharp")]
      S::CSharp => CSharp.$method($($pname,)*),
      #[cfg(feature = "lang-css")]
      S::Css => Css.$method($($pname,)*),
      #[cfg(feature = "lang-elixir")]
      S::Elixir => Elixir.$method($($pname,)*),
      #[cfg(feature = "lang-go")]
      S::Go => Go.$method($($pname,)*),
      #[cfg(feature = "lang-haskell")]
      S::Haskell => Haskell.$method($($pname,)*),
      #[cfg(feature = "lang-hcl")]
      S::Hcl => Hcl.$method($($pname,)*),
      #[cfg(feature = "lang-html")]
      S::Html => Html.$method($($pname,)*),
      #[cfg(feature = "lang-java")]
      S::Java => Java.$method($($pname,)*),
      #[cfg(feature = "lang-javascript")]
      S::JavaScript => JavaScript.$method($($pname,)*),
      #[cfg(feature = "lang-json")]
      S::Json => Json.$method($($pname,)*),
      #[cfg(feature = "lang-kotlin")]
      S::Kotlin => Kotlin.$method($($pname,)*),
      #[cfg(feature = "lang-lua")]
      S::Lua => Lua.$method($($pname,)*),
      #[cfg(feature = "lang-nix")]
      S::Nix => Nix.$method($($pname,)*),
      #[cfg(feature = "lang-php")]
      S::Php => Php.$method($($pname,)*),
      #[cfg(feature = "lang-python")]
      S::Python => Python.$method($($pname,)*),
      #[cfg(feature = "lang-ruby")]
      S::Ruby => Ruby.$method($($pname,)*),
      #[cfg(feature = "lang-rust")]
      S::Rust => Rust.$method($($pname,)*),
      #[cfg(feature = "lang-scala")]
      S::Scala => Scala.$method($($pname,)*),
      #[cfg(feature = "lang-solidity")]
      S::Solidity => Solidity.$method($($pname,)*),
      #[cfg(feature = "lang-swift")]
      S::Swift => Swift.$method($($pname,)*),
      #[cfg(feature = "lang-typescript")]
      S::Tsx => Tsx.$method($($pname,)*),
      #[cfg(feature = "lang-typescript")]
      S::TypeScript => TypeScript.$method($($pname,)*),
      #[cfg(feature = "lang-yaml")]
      S::Yaml => Yaml.$method($($pname,)*),
      // Catch-all for when no languages are enabled - this should never be reached
      // as SupportLang would have no variants, but needed for pattern matching completeness
      #[allow(unreachable_patterns)]
      _ => unreachable!("No language features enabled"),
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
  impl_lang_method!(kind_to_id, (kind: &str) => u16);
  impl_lang_method!(field_to_id, (field: &str) => Option<u16>);
  impl_lang_method!(meta_var_char, () => char);
  impl_lang_method!(expando_char, () => char);
  impl_lang_method!(extract_meta_var, (source: &str) => Option<MetaVariable>);
  impl_lang_method!(build_pattern, (builder: &PatternBuilder) => Result<Pattern, PatternError>);
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    execute_lang_method! { self, pre_process_pattern, query }
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
    #[allow(unused_variables)] root: Node<StrDoc<L>>,
  ) -> HashMap<String, Vec<TSRange>> {
    match self {
      #[cfg(feature = "lang-html")]
      SupportLang::Html => Html.extract_injections(root),
      _ => HashMap::new(),
    }
  }
}

fn extensions(lang: SupportLang) -> &'static [&'static str] {
  #[allow(unused)]
  use SupportLang::*;
  match lang {
    #[cfg(feature = "lang-bash")]
    Bash => &[
      "bash", "bats", "cgi", "command", "env", "fcgi", "ksh", "sh", "tmux", "tool", "zsh",
    ],
    #[cfg(feature = "lang-c")]
    C => &["c", "h"],
    #[cfg(feature = "lang-cpp")]
    Cpp => &["cc", "hpp", "cpp", "c++", "hh", "cxx", "cu", "ino"],
    #[cfg(feature = "lang-csharp")]
    CSharp => &["cs"],
    #[cfg(feature = "lang-css")]
    Css => &["css", "scss"],
    #[cfg(feature = "lang-elixir")]
    Elixir => &["ex", "exs"],
    #[cfg(feature = "lang-go")]
    Go => &["go"],
    #[cfg(feature = "lang-haskell")]
    Haskell => &["hs"],
    #[cfg(feature = "lang-hcl")]
    Hcl => &["hcl"],
    #[cfg(feature = "lang-html")]
    Html => &["html", "htm", "xhtml"],
    #[cfg(feature = "lang-java")]
    Java => &["java"],
    #[cfg(feature = "lang-javascript")]
    JavaScript => &["cjs", "js", "mjs", "jsx"],
    #[cfg(feature = "lang-json")]
    Json => &["json"],
    #[cfg(feature = "lang-kotlin")]
    Kotlin => &["kt", "ktm", "kts"],
    #[cfg(feature = "lang-lua")]
    Lua => &["lua"],
    #[cfg(feature = "lang-nix")]
    Nix => &["nix"],
    #[cfg(feature = "lang-php")]
    Php => &["php"],
    #[cfg(feature = "lang-python")]
    Python => &["py", "py3", "pyi", "bzl"],
    #[cfg(feature = "lang-ruby")]
    Ruby => &["rb", "rbw", "gemspec"],
    #[cfg(feature = "lang-rust")]
    Rust => &["rs"],
    #[cfg(feature = "lang-scala")]
    Scala => &["scala", "sc", "sbt"],
    #[cfg(feature = "lang-solidity")]
    Solidity => &["sol"],
    #[cfg(feature = "lang-swift")]
    Swift => &["swift"],
    #[cfg(feature = "lang-typescript")]
    TypeScript => &["ts", "cts", "mts"],
    #[cfg(feature = "lang-typescript")]
    Tsx => &["tsx"],
    #[cfg(feature = "lang-yaml")]
    Yaml => &["yaml", "yml"],
    #[allow(unreachable_patterns)]
    _ => unreachable!("No language features enabled"),
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
  use ast_grep_core::{matcher::MatcherExt, Pattern};

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
    assert!(source
      .replace(pattern, replacer)
      .expect("should parse successfully"));
    source.generate()
  }

  #[test]
  fn test_js_string() {
    test_match_lang("'a'", "'a'", JavaScript);
    test_match_lang("\"\"", "\"\"", JavaScript);
    test_match_lang("''", "''", JavaScript);
  }

  #[test]
  fn test_guess_by_extension() {
    let path = Path::new("foo.rs");
    assert_eq!(from_extension(path), Some(SupportLang::Rust));
  }

  // TODO: add test for file_types
}
