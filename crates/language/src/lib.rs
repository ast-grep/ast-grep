//! This module defines the supported programming languages for ast-grep.
//!
//! It provides a set of customized languages with expando_char / pre_process_pattern,
//! and a set of stub languages without preprocessing.
//! A rule of thumb: if your language does not accept identifiers like `$VAR`.
//! Use `define_language!` with an `expando` character for those languages; omit
//! it for languages that accept `$`. Each declaration also supplies its parser,
//! feature, aliases, extensions, and optional module for tests or custom behavior.
//! Add its name to `define_registry!` to include it in the built-in registry.
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

define_language! {
  Bash {
    feature: "tree-sitter-bash",
    module: bash,
    parser: tree_sitter_bash::LANGUAGE,
    aliases: ["bash"],
    extensions: [
      "bash", "bats", "cgi", "command", "env", "fcgi", "ksh", "sh", "tmux", "tool", "zsh",
    ],
  }
}

// https://en.cppreference.com/w/cpp/language/identifiers
define_language! {
  C {
    feature: "tree-sitter-c",
    parser: tree_sitter_c::LANGUAGE,
    aliases: ["c"],
    extensions: ["c", "h"],
    expando: '𐀀',
  }
}

define_language! {
  Cpp {
    feature: "tree-sitter-cpp",
    module: cpp,
    parser: tree_sitter_cpp::LANGUAGE,
    aliases: ["cc", "c++", "cpp", "cxx"],
    extensions: ["cc", "hpp", "cpp", "c++", "hh", "cxx", "cu", "ino"],
    expando: '𐀀',
  }
}

// https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#643-identifiers
// all letter number is accepted
// https://www.compart.com/en/unicode/category/Nl
define_language! {
  CSharp {
    feature: "tree-sitter-c-sharp",
    module: csharp,
    parser: tree_sitter_c_sharp::LANGUAGE,
    aliases: ["cs", "csharp"],
    extensions: ["cs"],
    expando: 'µ',
  }
}

// https://www.w3.org/TR/CSS21/grammar.html#scanner
define_language! {
  Css {
    feature: "tree-sitter-css",
    module: css,
    parser: tree_sitter_css::LANGUAGE,
    aliases: ["css"],
    extensions: ["css", "scss"],
    expando: '_',
  }
}

define_language! {
  Dart {
    feature: "tree-sitter-dart",
    module: dart,
    parser: tree_sitter_dart::LANGUAGE,
    aliases: ["dart"],
    extensions: ["dart"],
  }
}

// https://github.com/elixir-lang/tree-sitter-elixir/blob/a2861e88a730287a60c11ea9299c033c7d076e30/grammar.js#L245
define_language! {
  Elixir {
    feature: "tree-sitter-elixir",
    module: elixir,
    parser: tree_sitter_elixir::LANGUAGE,
    aliases: ["ex", "elixir"],
    extensions: ["ex", "exs"],
    expando: 'µ',
  }
}

// we can use any Unicode code point categorized as "Letter"
// https://go.dev/ref/spec#letter
define_language! {
  Go {
    feature: "tree-sitter-go",
    module: go,
    parser: tree_sitter_go::LANGUAGE,
    aliases: ["go", "golang"],
    extensions: ["go"],
    expando: 'µ',
  }
}

// GHC supports Unicode syntax per
// https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/unicode_syntax.html
// and the tree-sitter-haskell grammar parses it too.
define_language! {
  Haskell {
    feature: "tree-sitter-haskell",
    module: haskell,
    parser: tree_sitter_haskell::LANGUAGE,
    aliases: ["hs", "haskell"],
    extensions: ["hs"],
    expando: 'µ',
  }
}

// https://developer.hashicorp.com/terraform/language/syntax/configuration#identifiers
define_language! {
  Hcl {
    feature: "tree-sitter-hcl",
    module: hcl,
    parser: tree_sitter_hcl::LANGUAGE,
    aliases: ["hcl"],
    extensions: ["hcl", "nomad", "tf", "tfvars", "workflow"],
    expando: 'µ',
  }
}

define_language! {
  Html {
    feature: "tree-sitter-html",
    module: html,
    implementation: html::Html,
    aliases: ["html"],
    extensions: ["html", "htm", "xhtml"],
  }
}

define_language! {
  Java {
    feature: "tree-sitter-java",
    parser: tree_sitter_java::LANGUAGE,
    aliases: ["java"],
    extensions: ["java"],
  }
}

define_language! {
  JavaScript {
    feature: "tree-sitter-javascript",
    parser: tree_sitter_javascript::LANGUAGE,
    aliases: ["javascript", "js", "jsx"],
    extensions: ["cjs", "js", "mjs", "jsx"],
  }
}

define_language! {
  Json {
    feature: "tree-sitter-json",
    module: json,
    parser: tree_sitter_json::LANGUAGE,
    aliases: ["json"],
    extensions: ["json"],
  }
}

// https://github.com/fwcd/tree-sitter-kotlin/pull/93
define_language! {
  Kotlin {
    feature: "tree-sitter-kotlin",
    module: kotlin,
    parser: tree_sitter_kotlin::LANGUAGE,
    aliases: ["kotlin", "kt"],
    extensions: ["kt", "ktm", "kts"],
    expando: 'µ',
  }
}

define_language! {
  Lua {
    feature: "tree-sitter-lua",
    module: lua,
    parser: tree_sitter_lua::LANGUAGE,
    aliases: ["lua"],
    extensions: ["lua"],
  }
}

define_language! {
  Markdown {
    feature: "tree-sitter-md",
    module: markdown,
    parser: tree_sitter_md::LANGUAGE,
    aliases: ["markdown", "md"],
    extensions: ["markdown", "md"],
  }
}

// Nix uses $ for string interpolation (e.g., "${pkgs.hello}")
define_language! {
  Nix {
    feature: "tree-sitter-nix",
    module: nix,
    parser: tree_sitter_nix::LANGUAGE,
    aliases: ["nix"],
    extensions: ["nix"],
    expando: '_',
  }
}

// PHP accepts unicode to be used as some name not var name though
define_language! {
  Php {
    feature: "tree-sitter-php",
    module: php,
    parser: tree_sitter_php::LANGUAGE_PHP_ONLY,
    aliases: ["php"],
    extensions: ["php"],
    expando: 'µ',
  }
}

// we can use any char in unicode range [:XID_Start:]
// https://docs.python.org/3/reference/lexical_analysis.html#identifiers
// see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
define_language! {
  Python {
    feature: "tree-sitter-python",
    module: python,
    parser: tree_sitter_python::LANGUAGE,
    aliases: ["py", "python"],
    extensions: ["py", "py3", "pyi", "bzl", "bazel"],
    expando: 'µ',
  }
}

// https://github.com/tree-sitter/tree-sitter-ruby/blob/f257f3f57833d584050336921773738a3fd8ca22/grammar.js#L30C26-L30C78
define_language! {
  Ruby {
    feature: "tree-sitter-ruby",
    module: ruby,
    parser: tree_sitter_ruby::LANGUAGE,
    aliases: ["rb", "ruby"],
    extensions: ["rb", "rbw", "gemspec"],
    expando: 'µ',
  }
}

// we can use any char in unicode range [:XID_Start:]
// https://doc.rust-lang.org/reference/identifiers.html
define_language! {
  Rust {
    feature: "tree-sitter-rust",
    module: rust,
    parser: tree_sitter_rust::LANGUAGE,
    aliases: ["rs", "rust"],
    extensions: ["rs"],
    expando: 'µ',
  }
}

define_language! {
  Scala {
    feature: "tree-sitter-scala",
    module: scala,
    parser: tree_sitter_scala::LANGUAGE,
    aliases: ["scala"],
    extensions: ["scala", "sc", "sbt"],
  }
}

define_language! {
  Solidity {
    feature: "tree-sitter-solidity",
    module: solidity,
    parser: tree_sitter_solidity::LANGUAGE,
    aliases: ["sol", "solidity"],
    extensions: ["sol"],
  }
}

//https://docs.swift.org/swift-book/documentation/the-swift-programming-language/lexicalstructure/#Identifiers
define_language! {
  Swift {
    feature: "tree-sitter-swift",
    module: swift,
    parser: tree_sitter_swift::LANGUAGE,
    aliases: ["swift"],
    extensions: ["swift"],
    expando: 'µ',
  }
}

define_language! {
  Tsx {
    feature: "tree-sitter-typescript",
    parser: tree_sitter_typescript::LANGUAGE_TSX,
    aliases: ["tsx"],
    extensions: ["tsx"],
  }
}

define_language! {
  TypeScript {
    feature: "tree-sitter-typescript",
    parser: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
    aliases: ["ts", "typescript"],
    extensions: ["ts", "cts", "mts"],
  }
}

define_language! {
  Yaml {
    feature: "tree-sitter-yaml",
    module: yaml,
    parser: tree_sitter_yaml::LANGUAGE,
    aliases: ["yaml", "yml"],
    extensions: ["yaml", "yml"],
  }
}

// Zig identifiers are [A-Za-z_][A-Za-z0-9_]*; '$' is not one of them.
// https://ziglang.org/documentation/master/#Identifiers
define_language! {
  Zig {
    feature: "tree-sitter-zig",
    module: zig,
    parser: tree_sitter_zig::LANGUAGE,
    aliases: ["zig"],
    extensions: ["zig"],
    expando: '_',
  }
}

define_registry! {
  Bash, C, Cpp, CSharp, Css, Dart, Elixir, Go, Haskell, Hcl, Html, Java, JavaScript, Json,
  Kotlin, Lua, Markdown, Nix, Php, Python, Ruby, Rust, Scala, Solidity, Swift, Tsx, TypeScript,
  Yaml, Zig,
}

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
      execute_lang_method!{ self, $method, ($($pname),*) }
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
    execute_lang_method! { self, pre_process_pattern, (_query) }
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
    execute_lang_method! { self, extract_injections, (_root) }
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
