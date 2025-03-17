//! This module defines the supported programming languages for ast-grep.
//!
//! It provides a set of customized languages with expando_char / pre_process_pattern,
//! and a set of stub languages without preprocessing.
//! A rule of thumb: if your language does not accept identifiers like `$VAR`.
//! You need use `impl_lang_expando!` macro and a standalone file for testing.
//! Otherwise, you can define it as a stub language using `impl_lang!`.
//! To see the full list of languages, visit `<https://ast-grep.github.io/reference/languages.html>`

mod bash;
mod cpp;
mod csharp;
mod css;
mod elixir;
mod go;
mod haskell;
mod html;
mod json;
mod kotlin;
mod lua;
mod parsers;
mod php;
mod python;
mod ruby;
mod rust;
mod scala;
mod swift;
mod yaml;

pub use html::Html;

use ast_grep_core::language::{TSLanguage, TSRange};
use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::{Doc, Node};
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

pub use ast_grep_core::Language;

/// this macro implements bare-bone methods for a language
macro_rules! impl_lang {
  ($lang: ident, $func: ident) => {
    #[derive(Clone, Copy, Debug)]
    pub struct $lang;
    impl Language for $lang {
      fn get_ts_language(&self) -> TSLanguage {
        parsers::$func().into()
      }
    }
  };
}

fn pre_process_pattern(expando: char, query: &str) -> std::borrow::Cow<str> {
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
macro_rules! impl_lang_expando {
  ($lang: ident, $func: ident, $char: expr) => {
    #[derive(Clone, Copy, Debug)]
    pub struct $lang;
    impl ast_grep_core::language::Language for $lang {
      fn get_ts_language(&self) -> ast_grep_core::language::TSLanguage {
        $crate::parsers::$func().into()
      }
      fn expando_char(&self) -> char {
        $char
      }
      fn pre_process_pattern<'q>(&self, query: &'q str) -> std::borrow::Cow<'q, str> {
        pre_process_pattern(self.expando_char(), query)
      }
    }
  };
}

/// Implements the `ALIAS` associated constant for the given lang, which is
/// then used to define the `alias` const fn and a `Deserialize` impl.
macro_rules! impl_alias {
  ($lang:ident => $as:expr) => {
    impl $lang {
      pub const ALIAS: &'static [&'static str] = $as;
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
  ($($lang:ident => $as:expr),* $(,)?) => {
    $(impl_alias!($lang => $as);)*
    const fn alias(lang: SupportLang) -> &'static [&'static str] {
      match lang {
        $(SupportLang::$lang => $lang::ALIAS),*
      }
    }
  };
}

/* Customized Language with expando_char / pre_process_pattern */
// https://en.cppreference.com/w/cpp/language/identifiers
// Due to some issues in the tree-sitter parser, it is not possible to use
// unicode literals in identifiers for C/C++ parsers
impl_lang_expando!(C, language_c, '_');
impl_lang_expando!(Cpp, language_cpp, '_');
// https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#643-identifiers
// all letter number is accepted
// https://www.compart.com/en/unicode/category/Nl
impl_lang_expando!(CSharp, language_c_sharp, 'µ');
// https://www.w3.org/TR/CSS21/grammar.html#scanner
impl_lang_expando!(Css, language_css, '_');
// https://github.com/elixir-lang/tree-sitter-elixir/blob/a2861e88a730287a60c11ea9299c033c7d076e30/grammar.js#L245
impl_lang_expando!(Elixir, language_elixir, 'µ');
// we can use any Unicode code point categorized as "Letter"
// https://go.dev/ref/spec#letter
impl_lang_expando!(Go, language_go, 'µ');
// GHC supports Unicode syntax per
// https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/unicode_syntax.html
// and the tree-sitter-haskell grammar parses it too.
impl_lang_expando!(Haskell, language_haskell, 'µ');
// https://github.com/fwcd/tree-sitter-kotlin/pull/93
impl_lang_expando!(Kotlin, language_kotlin, 'µ');
// PHP accepts unicode to be used as some name not var name though
impl_lang_expando!(Php, language_php, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://docs.python.org/3/reference/lexical_analysis.html#identifiers
// see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
impl_lang_expando!(Python, language_python, 'µ');
// https://github.com/tree-sitter/tree-sitter-ruby/blob/f257f3f57833d584050336921773738a3fd8ca22/grammar.js#L30C26-L30C78
impl_lang_expando!(Ruby, language_ruby, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://doc.rust-lang.org/reference/identifiers.html
impl_lang_expando!(Rust, language_rust, 'µ');
//https://docs.swift.org/swift-book/documentation/the-swift-programming-language/lexicalstructure/#Identifiers
impl_lang_expando!(Swift, language_swift, 'µ');

// Stub Language without preprocessing
// Language Name, tree-sitter-name, alias, extension
impl_lang!(Bash, language_bash);
impl_lang!(Java, language_java);
impl_lang!(JavaScript, language_javascript);
impl_lang!(Json, language_json);
impl_lang!(Lua, language_lua);
impl_lang!(Scala, language_scala);
impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);
impl_lang!(Yaml, language_yaml);
// See ripgrep for extensions
// https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/default_types.rs

/// Represents all built-in languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
pub enum SupportLang {
  Bash,
  C,
  Cpp,
  CSharp,
  Css,
  Go,
  Elixir,
  Haskell,
  Html,
  Java,
  JavaScript,
  Json,
  Kotlin,
  Lua,
  Php,
  Python,
  Ruby,
  Rust,
  Scala,
  Swift,
  Tsx,
  TypeScript,
  Yaml,
}

impl SupportLang {
  pub const fn all_langs() -> &'static [SupportLang] {
    use SupportLang::*;
    &[
      Bash, C, Cpp, CSharp, Css, Elixir, Go, Haskell, Html, Java, JavaScript, Json, Kotlin, Lua,
      Php, Python, Ruby, Rust, Scala, Swift, Tsx, TypeScript, Yaml,
    ]
  }

  pub fn file_types(&self) -> Types {
    file_types(*self)
  }
}

impl fmt::Display for SupportLang {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
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
      LanguageNotSupported(lang) => write!(f, "{} is not supported!", lang),
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

impl_aliases! {
  Bash => &["bash"],
  C => &["c"],
  Cpp => &["cc", "c++", "cpp", "cxx"],
  CSharp => &["cs", "csharp"],
  Css => &["css"],
  Elixir => &["ex", "elixir"],
  Go => &["go", "golang"],
  Haskell => &["hs", "haskell"],
  Html => &["html"],
  Java => &["java"],
  JavaScript => &["javascript", "js", "jsx"],
  Json => &["json"],
  Kotlin => &["kotlin", "kt"],
  Lua => &["lua"],
  Php => &["php"],
  Python => &["py", "python"],
  Ruby => &["rb", "ruby"],
  Rust => &["rs", "rust"],
  Scala => &["scala"],
  Swift => &["swift"],
  TypeScript => &["ts", "typescript"],
  Tsx => &["tsx"],
  Yaml => &["yaml", "yml"],
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
    use SupportLang as S;
    match $me {
      S::Bash => Bash.$method($($pname,)*),
      S::C => C.$method($($pname,)*),
      S::Cpp => Cpp.$method($($pname,)*),
      S::CSharp => CSharp.$method($($pname,)*),
      S::Css => Css.$method($($pname,)*),
      S::Elixir => Elixir.$method($($pname,)*),
      S::Go => Go.$method($($pname,)*),
      S::Haskell => Haskell.$method($($pname,)*),
      S::Html => Html.$method($($pname,)*),
      S::Java => Java.$method($($pname,)*),
      S::JavaScript => JavaScript.$method($($pname,)*),
      S::Json => Json.$method($($pname,)*),
      S::Kotlin => Kotlin.$method($($pname,)*),
      S::Lua => Lua.$method($($pname,)*),
      S::Php => Php.$method($($pname,)*),
      S::Python => Python.$method($($pname,)*),
      S::Ruby => Ruby.$method($($pname,)*),
      S::Rust => Rust.$method($($pname,)*),
      S::Scala => Scala.$method($($pname,)*),
      S::Swift => Swift.$method($($pname,)*),
      S::Tsx => Tsx.$method($($pname,)*),
      S::TypeScript => TypeScript.$method($($pname,)*),
      S::Yaml => Yaml.$method($($pname,)*),
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
  fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
    from_extension(path.as_ref())
  }

  impl_lang_method!(get_ts_language, () => TSLanguage);
  impl_lang_method!(meta_var_char, () => char);
  impl_lang_method!(expando_char, () => char);
  impl_lang_method!(extract_meta_var, (source: &str) => Option<MetaVariable>);
  impl_lang_method!(injectable_languages, () => Option<&'static [&'static str]>);

  fn extract_injections<D: Doc>(&self, root: Node<D>) -> HashMap<String, Vec<TSRange>> {
    match self {
      SupportLang::Html => Html.extract_injections(root),
      _ => HashMap::new(),
    }
  }

  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    execute_lang_method! { self, pre_process_pattern, query }
  }
}

fn extensions(lang: SupportLang) -> &'static [&'static str] {
  use SupportLang::*;
  match lang {
    Bash => &[
      "bash", "bats", "cgi", "command", "env", "fcgi", "ksh", "sh", "tmux", "tool", "zsh",
    ],
    C => &["c", "h"],
    Cpp => &["cc", "hpp", "cpp", "c++", "hh", "cxx", "cu", "ino"],
    CSharp => &["cs"],
    Css => &["css", "scss"],
    Elixir => &["ex", "exs"],
    Go => &["go"],
    Haskell => &["hs"],
    Html => &["html", "htm", "xhtml"],
    Java => &["java"],
    JavaScript => &["cjs", "js", "mjs", "jsx"],
    Json => &["json"],
    Kotlin => &["kt", "ktm", "kts"],
    Lua => &["lua"],
    Php => &["php"],
    Python => &["py", "py3", "pyi", "bzl"],
    Ruby => &["rb", "rbw", "gemspec"],
    Rust => &["rs"],
    Scala => &["scala", "sc", "sbt"],
    Swift => &["swift"],
    TypeScript => &["ts", "cts", "mts"],
    Tsx => &["tsx"],
    Yaml => &["yaml", "yml"],
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
  use ast_grep_core::{matcher::MatcherExt, source::TSParseError, Pattern};

  pub fn test_match_lang(query: &str, source: &str, lang: impl Language) {
    let cand = lang.ast_grep(source);
    let pattern = Pattern::str(query, lang);
    assert!(
      pattern.find_node(cand.root()).is_some(),
      "goal: {pattern:?}, candidate: {}",
      cand.root().to_sexp(),
    );
  }

  pub fn test_non_match_lang(query: &str, source: &str, lang: impl Language) {
    let cand = lang.ast_grep(source);
    let pattern = Pattern::str(query, lang);
    assert!(
      pattern.find_node(cand.root()).is_none(),
      "goal: {pattern:?}, candidate: {}",
      cand.root().to_sexp(),
    );
  }

  pub fn test_replace_lang(
    src: &str,
    pattern: &str,
    replacer: &str,
    lang: impl Language,
  ) -> Result<String, TSParseError> {
    let mut source = lang.ast_grep(src);
    // TODO: we should have still have pattern as replacer
    let replacer = lang.pre_process_pattern(replacer);
    let replacer = lang.ast_grep(replacer).inner;
    assert!(source.replace(pattern, replacer)?);
    Ok(source.generate())
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
