//! This module defines the supported programming languages for ast-grep.
//!
//! It provides a set of customized languages with expando_char / pre_process_pattern,
//! and a set of stub languages without preprocessing.
//! A rule of thumb: if your language does not accept identifiers like `$VAR`.
//! You need use `impl_lang_expando!` macro and a standalone file for testing.
//! Otherwise, you can define it as a stub language using `impl_lang!`.
//! To see the full list of languages, visit `<https://ast-grep.github.io/reference/languages.html>`

mod cpp;
mod csharp;
mod css;
mod go;
mod json;
mod kotlin;
mod lua;
mod parsers;
mod python;
mod rust;
mod scala;

use ast_grep_core::language::TSLanguage;
use ast_grep_core::meta_var::MetaVariable;
use ignore::types::{Types, TypesBuilder};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;

pub use ast_grep_core::Language;

/// this macro implements bare-bone methods for a language
macro_rules! impl_lang {
  ($lang: ident, $func: ident) => {
    #[derive(Clone, Copy)]
    pub struct $lang;
    impl Language for $lang {
      fn get_ts_language(&self) -> TSLanguage {
        parsers::$func().into()
      }
    }
  };
}

/// this macro will implement expando_char and pre_process_pattern
/// use this if your language does not accept $ as valid identifier char
macro_rules! impl_lang_expando {
  ($lang: ident, $func: ident, $char: expr) => {
    #[derive(Clone, Copy)]
    pub struct $lang;
    impl ast_grep_core::language::Language for $lang {
      fn get_ts_language(&self) -> ast_grep_core::language::TSLanguage {
        $crate::parsers::$func().into()
      }
      fn expando_char(&self) -> char {
        $char
      }
      fn pre_process_pattern<'q>(&self, query: &'q str) -> std::borrow::Cow<'q, str> {
        // use stack buffer to reduce allocation
        let mut buf = [0; 4];
        let expando = self.expando_char().encode_utf8(&mut buf);
        // TODO: use more precise replacement
        let replaced = query.replace(self.meta_var_char(), expando);
        std::borrow::Cow::Owned(replaced)
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
// we can use any Unicode code point categorized as "Letter"
// https://go.dev/ref/spec#letter
impl_lang_expando!(Go, language_go, 'µ');
// https://github.com/fwcd/tree-sitter-kotlin/pull/93
impl_lang_expando!(Kotlin, language_kotlin, '_');
// we can use any char in unicode range [:XID_Start:]
// https://docs.python.org/3/reference/lexical_analysis.html#identifiers
// see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
impl_lang_expando!(Python, language_python, 'µ');
// https://github.com/tree-sitter/tree-sitter-ruby/blob/f257f3f57833d584050336921773738a3fd8ca22/grammar.js#L30C26-L30C78
impl_lang_expando!(Ruby, language_ruby, 'µ');
// we can use any char in unicode range [:XID_Start:]
// https://doc.rust-lang.org/reference/identifiers.html
impl_lang_expando!(Rust, language_rust, 'µ');

// Stub Language without preprocessing
// Language Name, tree-sitter-name, alias, extension
impl_lang!(Dart, language_dart);
impl_lang!(Html, language_html);
impl_lang!(Java, language_java);
impl_lang!(JavaScript, language_javascript);
impl_lang!(Json, language_json);
impl_lang!(Lua, language_lua);
impl_lang!(Scala, language_scala);
impl_lang!(Swift, language_swift);
impl_lang!(Thrift, language_thrift);
impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);
// See ripgrep for extensions
// https://github.com/BurntSushi/ripgrep/blob/master/crates/ignore/src/default_types.rs

/// Represents all built-in languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
pub enum SupportLang {
  C,
  Cpp,
  CSharp,
  Css,
  Dart,
  Go,
  Html,
  Java,
  JavaScript,
  Json,
  Kotlin,
  Lua,
  Python,
  Ruby,
  Rust,
  Scala,
  Swift,
  Thrift,
  Tsx,
  TypeScript,
}

impl SupportLang {
  pub const fn all_langs() -> &'static [SupportLang] {
    use SupportLang::*;
    &[
      C, Cpp, CSharp, Css, Dart, Go, Html, Java, JavaScript, Json, Kotlin, Lua, Python, Ruby, Rust,
      Scala, Swift, Thrift, Tsx, TypeScript,
    ]
  }

  pub fn file_types(&self) -> Types {
    file_types(self)
  }
}

impl fmt::Display for SupportLang {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    let s = String::deserialize(deserializer)?;
    FromStr::from_str(&s).map_err(de::Error::custom)
  }
}

const fn alias(lang: &SupportLang) -> &[&str] {
  use SupportLang::*;
  match lang {
    C => &["c"],
    Cpp => &["cc", "c++", "cpp", "cxx"],
    CSharp => &["cs", "csharp"],
    Css => &["css"],
    Dart => &["dart"],
    Go => &["go", "golang"],
    Html => &["html"],
    Java => &["java"],
    JavaScript => &["javascript", "js", "jsx"],
    Json => &["json"],
    Kotlin => &["kotlin", "kt"],
    Lua => &["lua"],
    Python => &["py", "python"],
    Ruby => &["rb", "ruby"],
    Rust => &["rs", "rust"],
    Scala => &["scala"],
    Swift => &["swift"],
    Thrift => &["thrift"],
    TypeScript => &["ts", "typescript"],
    Tsx => &["tsx"],
  }
}

/// Implements the language names and aliases.
impl FromStr for SupportLang {
  type Err = SupportLangErr;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    for lang in Self::all_langs() {
      for moniker in alias(lang) {
        if s.eq_ignore_ascii_case(moniker) {
          return Ok(*lang);
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
      S::C => C.$method($($pname,)*),
      S::Cpp => Cpp.$method($($pname,)*),
      S::CSharp => CSharp.$method($($pname,)*),
      S::Css => Css.$method($($pname,)*),
      S::Dart => Dart.$method($($pname,)*),
      S::Go => Go.$method($($pname,)*),
      S::Html => Html.$method($($pname,)*),
      S::Java => Java.$method($($pname,)*),
      S::JavaScript => JavaScript.$method($($pname,)*),
      S::Json => Json.$method($($pname,)*),
      S::Kotlin => Kotlin.$method($($pname,)*),
      S::Lua => Lua.$method($($pname,)*),
      S::Python => Python.$method($($pname,)*),
      S::Ruby => Ruby.$method($($pname,)*),
      S::Rust => Rust.$method($($pname,)*),
      S::Scala => Scala.$method($($pname,)*),
      S::Swift => Swift.$method($($pname,)*),
      S::Thrift => Thrift.$method($($pname,)*),
      S::Tsx => Tsx.$method($($pname,)*),
      S::TypeScript => TypeScript.$method($($pname,)*),
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

  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    execute_lang_method! { self, pre_process_pattern, query }
  }
}

fn extensions(lang: &SupportLang) -> &[&str] {
  use SupportLang::*;
  match lang {
    C => &["c", "h"],
    Cpp => &["cc", "hpp", "cpp", "c++", "hh", "cxx", "cu", "ino"],
    CSharp => &["cs"],
    Css => &["css", "scss"],
    Dart => &["dart"],
    Go => &["go"],
    Html => &["html", "htm", "xhtml"],
    Java => &["java"],
    JavaScript => &["cjs", "js", "mjs", "jsx"],
    Json => &["json"],
    Kotlin => &["kt", "ktm", "kts"],
    Lua => &["lua"],
    Python => &["py", "py3", "pyi", "bzl"],
    Ruby => &["rb", "rbw", "gemspec"],
    Rust => &["rs"],
    Scala => &["scala", "sc", "sbt"],
    Swift => &["swift"],
    Thrift => &["thrift"],
    TypeScript => &["ts", "cts", "mts"],
    Tsx => &["tsx"],
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
    .find(|l| extensions(l).contains(&ext))
}

fn add_custom_file_type<'b>(
  builder: &'b mut TypesBuilder,
  file_type: &str,
  suffix_list: &[&str],
) -> &'b mut TypesBuilder {
  for suffix in suffix_list {
    builder
      .add(file_type, suffix)
      .expect("file pattern must compile");
  }
  builder.select(file_type)
}

fn file_types(lang: &SupportLang) -> Types {
  use SupportLang as L;
  let mut builder = TypesBuilder::new();
  builder.add_defaults();
  let builder = match lang {
    L::C => builder.select("c"),
    L::Cpp => builder.select("cpp"),
    L::CSharp => builder.select("csharp"),
    L::Css => builder.select("css"),
    L::Dart => builder.select("dart"),
    L::Go => builder.select("go"),
    L::Html => builder.select("html"),
    L::Java => builder.select("java"),
    L::JavaScript => {
      add_custom_file_type(&mut builder, "myjs", &["*.js", "*.cjs", "*.jsx", "*.mjs"])
    }
    L::Json => builder.select("json"),
    L::Kotlin => builder.select("kotlin"),
    L::Lua => builder.select("lua"),
    L::Python => builder.select("py"),
    L::Ruby => builder.select("ruby"),
    L::Rust => builder.select("rust"),
    L::Scala => builder.select("scala"),
    L::Swift => builder.select("swift"),
    L::Thrift => builder.select("thrift"),
    L::Tsx => {
      builder
        .add("mytsx", "*.tsx")
        .expect("file pattern must compile");
      builder.select("mytsx")
    }
    L::TypeScript => add_custom_file_type(&mut builder, "myts", &["*.ts", "*.cts", "*.mts"]),
  };
  builder.build().expect("file type must be valid")
}

pub fn config_file_type() -> Types {
  let mut builder = TypesBuilder::new();
  let builder = add_custom_file_type(&mut builder, "yml", &["*.yml", "*.yaml"]);
  builder.build().expect("yaml type must be valid")
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::{source::TSParseError, Matcher, Pattern};

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
    let replacer = Pattern::new(replacer, lang);
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
