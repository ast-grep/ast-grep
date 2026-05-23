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

mod bash;
mod cpp;
mod csharp;
mod css;
mod dart;
mod elixir;
mod go;
mod haskell;
mod hcl;
mod html;
mod json;
mod kotlin;
mod lua;
mod nix;
mod parsers;
mod php;
mod python;
mod ruby;
mod rust;
mod scala;
mod solidity;
mod swift;
mod toml;
mod yaml;

use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
pub use html::Html;

use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::tree_sitter::{StrDoc, TSLanguage, TSRange};
use ast_grep_core::Node;
use ignore::types::{Types, TypesBuilder};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::iter::repeat;
use std::path::Path;
use std::str::FromStr;

pub use ast_grep_core::language::Language;
pub use ast_grep_core::tree_sitter::LanguageExt;

/// this macro implements bare-bone methods for a language
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
impl_lang_expando!(C, language_c, '𐀀');
impl_lang_expando!(Cpp, language_cpp, '𐀀');
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
// https://developer.hashicorp.com/terraform/language/syntax/configuration#identifiers
impl_lang_expando!(Hcl, language_hcl, 'µ');
// https://github.com/fwcd/tree-sitter-kotlin/pull/93
impl_lang_expando!(Kotlin, language_kotlin, 'µ');
// Nix uses $ for string interpolation (e.g., "${pkgs.hello}")
impl_lang_expando!(Nix, language_nix, '_');
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
impl_lang!(Solidity, language_solidity);
impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);
impl_lang!(Dart, language_dart);

// TOML — bare keys only allow [A-Za-z0-9_-], so the expando char is '_'.
// `(string)` is declared atomic so that even an empty `""` pattern matches
// only `""` (not `"foo"`); the general "content absorbed into parent" check
// already covers non-empty cases.
#[derive(Clone, Copy, Debug)]
pub struct Toml;
impl Language for Toml {
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
    '_'
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    pre_process_pattern(self.expando_char(), query)
  }
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|src| StrDoc::try_new(src, *self))
  }
  fn kind_is_atomic(&self, kind_id: u16) -> bool {
    kind_id == self.kind_to_id("string")
  }
}
impl LanguageExt for Toml {
  fn get_ts_language(&self) -> TSLanguage {
    parsers::language_toml()
  }
}

// YAML — quoted scalars (`single_quote_scalar`, `double_quote_scalar`) and
// block scalars are atomic for the same reason as TOML strings: they're
// data literals where text-identity matters even when empty.
#[derive(Clone, Copy, Debug)]
pub struct Yaml;
impl Language for Yaml {
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
    builder.build(|src| StrDoc::try_new(src, *self))
  }
  fn kind_is_atomic(&self, kind_id: u16) -> bool {
    kind_id == self.kind_to_id("single_quote_scalar")
      || kind_id == self.kind_to_id("double_quote_scalar")
      || kind_id == self.kind_to_id("block_scalar")
  }
}
impl LanguageExt for Yaml {
  fn get_ts_language(&self) -> TSLanguage {
    parsers::language_yaml()
  }
}
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
  Dart,
  Go,
  Elixir,
  Haskell,
  Hcl,
  Html,
  Java,
  JavaScript,
  Json,
  Kotlin,
  Lua,
  Nix,
  Php,
  Python,
  Ruby,
  Rust,
  Scala,
  Solidity,
  Swift,
  Toml,
  Tsx,
  TypeScript,
  Yaml,
}

impl SupportLang {
  pub const fn all_langs() -> &'static [SupportLang] {
    use SupportLang::*;
    &[
      Bash, C, Cpp, CSharp, Css, Dart, Elixir, Go, Haskell, Hcl, Html, Java, JavaScript, Json,
      Kotlin, Lua, Nix, Php, Python, Ruby, Rust, Scala, Solidity, Swift, Toml, Tsx, TypeScript,
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

impl_aliases! {
  Bash => &["bash"],
  C => &["c"],
  Cpp => &["cc", "c++", "cpp", "cxx"],
  CSharp => &["cs", "csharp"],
  Css => &["css"],
  Dart => &["dart"],
  Elixir => &["ex", "elixir"],
  Go => &["go", "golang"],
  Haskell => &["hs", "haskell"],
  Hcl => &["hcl"],
  Html => &["html"],
  Java => &["java"],
  JavaScript => &["javascript", "js", "jsx"],
  Json => &["json"],
  Kotlin => &["kotlin", "kt"],
  Lua => &["lua"],
  Nix => &["nix"],
  Php => &["php"],
  Python => &["py", "python"],
  Ruby => &["rb", "ruby"],
  Rust => &["rs", "rust"],
  Scala => &["scala"],
  Solidity => &["sol", "solidity"],
  Swift => &["swift"],
  Toml => &["toml"],
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
      S::Dart => Dart.$method($($pname,)*),
      S::Elixir => Elixir.$method($($pname,)*),
      S::Go => Go.$method($($pname,)*),
      S::Haskell => Haskell.$method($($pname,)*),
      S::Hcl => Hcl.$method($($pname,)*),
      S::Html => Html.$method($($pname,)*),
      S::Java => Java.$method($($pname,)*),
      S::JavaScript => JavaScript.$method($($pname,)*),
      S::Json => Json.$method($($pname,)*),
      S::Kotlin => Kotlin.$method($($pname,)*),
      S::Lua => Lua.$method($($pname,)*),
      S::Nix => Nix.$method($($pname,)*),
      S::Php => Php.$method($($pname,)*),
      S::Python => Python.$method($($pname,)*),
      S::Ruby => Ruby.$method($($pname,)*),
      S::Rust => Rust.$method($($pname,)*),
      S::Scala => Scala.$method($($pname,)*),
      S::Solidity => Solidity.$method($($pname,)*),
      S::Swift => Swift.$method($($pname,)*),
      S::Toml => Toml.$method($($pname,)*),
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
  impl_lang_method!(kind_to_id, (kind: &str) => u16);
  impl_lang_method!(field_to_id, (field: &str) => Option<u16>);
  impl_lang_method!(meta_var_char, () => char);
  impl_lang_method!(expando_char, () => char);
  impl_lang_method!(extract_meta_var, (source: &str) => Option<MetaVariable>);
  impl_lang_method!(build_pattern, (builder: &PatternBuilder) => Result<Pattern, PatternError>);
  // Must delegate — the trait default returns `false`, which would silently
  // disable Toml/Yaml's atomic-kind overrides whenever a Doc is parameterized
  // by SupportLang (e.g. the CLI's StrDoc<SgLang> path).
  impl_lang_method!(kind_is_atomic, (kind_id: u16) => bool);
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
    root: Node<StrDoc<L>>,
  ) -> Vec<(String, Vec<TSRange>)> {
    match self {
      SupportLang::Html => Html.extract_injections(root),
      _ => Vec::new(),
    }
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
    Dart => &["dart"],
    Elixir => &["ex", "exs"],
    Go => &["go"],
    Haskell => &["hs"],
    Hcl => &["hcl", "nomad", "tf", "tfvars", "workflow"],
    Html => &["html", "htm", "xhtml"],
    Java => &["java"],
    JavaScript => &["cjs", "js", "mjs", "jsx"],
    Json => &["json"],
    Kotlin => &["kt", "ktm", "kts"],
    Lua => &["lua"],
    Nix => &["nix"],
    Php => &["php"],
    Python => &["py", "py3", "pyi", "bzl"],
    Ruby => &["rb", "rbw", "gemspec"],
    Rust => &["rs"],
    Scala => &["scala", "sc", "sbt"],
    Solidity => &["sol"],
    Swift => &["swift"],
    Toml => &["toml"],
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

  // --- Cross-language atom-like-node scan ---
  //
  // Reports every language whose AST contains a named node whose own text
  // differs from the concatenation of its children's text. That mismatch is
  // the structural signature of the TOML/YAML quoted-string bug: content is
  // folded into the parent's `text()` but no child node carries it, so the
  // pattern matcher (which compares kind + recurses into children) cannot
  // distinguish two different-content instances of the same kind.
  //
  // Run with: cargo test -p ast-grep-language atom_like -- --ignored --nocapture
  //
  // A finding here is a SUSPECT, not a proven bug; confirm with a real
  // test_non_match pair. False positives are possible when the difference is
  // pure whitespace between children.
  #[test]
  #[ignore]
  fn atom_like_node_scan_across_languages() {
    // (label, source) pairs probing common literal forms per language.
    fn probes_for(lang: SupportLang) -> Vec<(&'static str, &'static str)> {
      use SupportLang::*;
      match lang {
        Bash => vec![("dq", r#"x="foo""#), ("sq", "x='foo'"), ("raw", "x=foo")],
        C => vec![("dq", r#"char* x = "foo";"#), ("ch", "char c = 'a';")],
        Cpp => vec![
          ("dq", r#"auto x = "foo";"#),
          ("ch", "auto c = 'a';"),
          ("raw", r#"auto x = R"(foo)";"#),
        ],
        CSharp => vec![
          ("dq", r#"var x = "foo";"#),
          ("verbatim", r#"var x = @"foo";"#),
          ("interp", r#"var x = $"foo";"#),
        ],
        Css => vec![
          ("dq", "a { content: \"foo\"; }"),
          ("sq", "a { content: 'foo'; }"),
        ],
        Dart => vec![
          ("dq", r#"var x = "foo";"#),
          ("sq", "var x = 'foo';"),
          ("triple", r#"var x = """foo""";"#),
        ],
        Elixir => vec![
          ("dq", r#"x = "foo""#),
          ("charlist", "x = 'foo'"),
          ("sigil", r#"x = ~s"foo""#),
        ],
        Go => vec![
          ("dq", r#"x := "foo""#),
          ("raw", "x := `foo`"),
          ("rune", "x := 'a'"),
        ],
        Haskell => vec![("dq", r#"x = "foo""#), ("ch", "x = 'a'")],
        Hcl => vec![("dq", r#"x = "foo""#), ("heredoc", "x = <<EOT\nfoo\nEOT")],
        Html => vec![
          ("attr_dq", r#"<a href="foo">"#),
          ("attr_sq", "<a href='foo'>"),
          ("attr_unq", "<a href=foo>"),
          ("text", "<p>foo</p>"),
        ],
        Java => vec![("dq", r#"String x = "foo";"#), ("ch", "char c = 'a';")],
        JavaScript => vec![
          ("dq", r#"const x = "foo";"#),
          ("sq", "const x = 'foo';"),
          ("tpl", "const x = `foo`;"),
          ("regex", "const x = /foo/g;"),
        ],
        Json => vec![("dq", r#"{"k": "foo"}"#)],
        Kotlin => vec![
          ("dq", r#"val x = "foo""#),
          ("triple", "val x = \"\"\"foo\"\"\""),
          ("ch", "val c = 'a'"),
        ],
        Lua => vec![
          ("dq", r#"x = "foo""#),
          ("sq", "x = 'foo'"),
          ("long", "x = [[foo]]"),
        ],
        Nix => vec![("dq", r#"x = "foo";"#), ("ind", "x = ''foo'';")],
        Php => vec![
          ("dq", r#"<?php $x = "foo"; ?>"#),
          ("sq", "<?php $x = 'foo'; ?>"),
          ("heredoc", "<?php $x = <<<EOT\nfoo\nEOT;\n?>"),
        ],
        Python => vec![
          ("dq", r#"x = "foo""#),
          ("sq", "x = 'foo'"),
          ("triple", r#"x = """foo""""#),
          ("f", r#"x = f"foo""#),
        ],
        Ruby => vec![
          ("dq", r#"x = "foo""#),
          ("sq", "x = 'foo'"),
          ("sym", "x = :foo"),
          ("heredoc", "x = <<~EOT\n  foo\nEOT"),
        ],
        Rust => vec![
          ("dq", r#"let x = "foo";"#),
          ("ch", "let c = 'a';"),
          ("byte", r#"let x = b"foo";"#),
          ("raw", r##"let x = r#"foo"#;"##),
        ],
        Scala => vec![
          ("dq", r#"val x = "foo""#),
          ("triple", "val x = \"\"\"foo\"\"\""),
          ("ch", "val c = 'a'"),
        ],
        Solidity => vec![("dq", r#"string x = "foo";"#), ("sq", "string x = 'foo';")],
        Swift => vec![
          ("dq", r#"let x = "foo""#),
          ("triple", "let x = \"\"\"\nfoo\n\"\"\""),
        ],
        Toml => vec![
          ("dq", r#"x = "foo""#),
          ("sq", "x = 'foo'"),
          ("triple", r#"x = """foo""""#),
        ],
        Tsx => vec![("dq", r#"const x: string = "foo";"#)],
        TypeScript => vec![("dq", r#"const x: string = "foo";"#)],
        Yaml => vec![
          ("dq", r#"a: "foo""#),
          ("sq", "a: 'foo'"),
          ("plain", "a: foo"),
          ("block_lit", "a: |\n  foo\n"),
          ("block_fold", "a: >\n  foo\n"),
        ],
      }
    }

    fn scan<L: LanguageExt + Clone>(lang: L, label: &str, src: &str, out: &mut Vec<String>) {
      let g = lang.ast_grep(src);
      let root = g.root();
      walk(root, label, out);
    }

    fn walk<D: ast_grep_core::Doc>(
      node: ast_grep_core::Node<D>,
      label: &str,
      out: &mut Vec<String>,
    ) {
      if node.is_named() && !node.is_leaf() {
        let parent_text = node.text().to_string();
        let concat: String = node.children().map(|c| c.text().to_string()).collect();
        // Compare ignoring whitespace, since tree-sitter doesn't surface inter-token whitespace as a child
        let pt: String = parent_text.chars().filter(|c| !c.is_whitespace()).collect();
        let ct: String = concat.chars().filter(|c| !c.is_whitespace()).collect();
        if pt != ct {
          out.push(format!(
            "  [{}] kind={} text={:?} children_concat={:?}",
            label,
            node.kind(),
            parent_text,
            concat
          ));
        }
      }
      for c in node.children() {
        walk(c, label, out);
      }
    }

    println!("\n=== atom-like node scan across all builtin languages ===");
    let mut any = false;
    for &lang in SupportLang::all_langs() {
      let mut findings = Vec::new();
      for (label, src) in probes_for(lang) {
        scan(lang, label, src, &mut findings);
      }
      // dedupe identical (kind, label) lines
      findings.sort();
      findings.dedup();
      if !findings.is_empty() {
        any = true;
        println!("\n{:?}:", lang);
        for f in findings {
          println!("{}", f);
        }
      }
    }
    if !any {
      println!("no atom-like nodes detected");
    }
    println!("=== end scan ===\n");
    panic!("intentional fail to surface stdout");
  }

  // Detail-dump for suspects flagged by the cross-language scan.
  #[test]
  #[ignore]
  fn atom_like_node_detail_dump() {
    fn dump<L: LanguageExt + Clone>(lang: L, label: &str, src: &str) {
      println!("--- {label} ---");
      let g = lang.ast_grep(src);
      println!("sexp: {}", g.root().get_inner_node().to_sexp());
      fn walk<D: ast_grep_core::Doc>(n: ast_grep_core::Node<D>, indent: usize) {
        println!(
          "{:indent$}{} named={} text={:?}",
          "",
          n.kind(),
          n.is_named(),
          n.text(),
          indent = indent
        );
        for c in n.children() {
          walk(c, indent + 2);
        }
      }
      walk(g.root(), 0);
    }

    dump(Kotlin, "kotlin character_literal", "val c = 'a'");
    dump(Kotlin, "kotlin string_literal", r#"val x = "foo""#);
    dump(Rust, "rust raw_string_literal", r##"let x = r#"foo"#;"##);
    panic!("intentional fail to surface stdout");
  }
}
