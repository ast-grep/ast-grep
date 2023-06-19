mod cpp;
mod csharp;
mod css;
mod go;
mod parsers;
mod python;
mod rust;
use ignore::types::{Types, TypesBuilder};
use std::borrow::Cow;
use std::fmt;
use std::path::Path;

pub use cpp::Cpp;
pub use csharp::CSharp;
pub use css::Css;
pub use go::Go;
pub use python::Python;
pub use rust::Rust;

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

impl_lang!(C, language_c);
impl_lang!(Dart, language_dart);
impl_lang!(Html, language_html);
impl_lang!(Java, language_java);
impl_lang!(JavaScript, language_javascript);
impl_lang!(Kotlin, language_kotlin);
impl_lang!(Lua, language_lua);
impl_lang!(Swift, language_swift);
impl_lang!(Thrift, language_thrift);
impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);

use ast_grep_core::language::TSLanguage;
use ast_grep_core::meta_var::MetaVariable;
pub use ast_grep_core::Language;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// represents a dynamic language
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
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
  Kotlin,
  Lua,
  Python,
  Rust,
  Swift,
  Thrift,
  Tsx,
  TypeScript,
}

impl SupportLang {
  pub fn all_langs() -> Vec<SupportLang> {
    use SupportLang::*;
    vec![
      C, Cpp, CSharp, Css, Dart, Go, Html, Java, JavaScript, Kotlin, Lua, Python, Rust, Swift,
      Thrift, Tsx, TypeScript,
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

impl FromStr for SupportLang {
  type Err = SupportLangErr;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    use SupportLang::*;
    match s {
      "c" => Ok(C),
      "cc" | "c++" | "cpp" | "cxx" => Ok(Cpp),
      "cs" | "csharp" => Ok(CSharp),
      "css" | "scss" => Ok(Css),
      "dart" => Ok(Dart),
      "go" | "golang" => Ok(Go),
      "html" => Ok(Html),
      "java" => Ok(Java),
      "js" | "jsx" => Ok(JavaScript),
      "kt" | "ktm" | "kts" => Ok(Kotlin),
      "lua" => Ok(Lua),
      "py" | "python" => Ok(Python),
      "rs" | "rust" => Ok(Rust),
      "swift" => Ok(Swift),
      "thrift" => Ok(Thrift),
      "ts" => Ok(TypeScript),
      "tsx" => Ok(Tsx),
      _ => Err(SupportLangErr::LanguageNotSupported(s.to_string())),
    }
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
      S::Kotlin => Kotlin.$method($($pname,)*),
      S::Lua => Lua.$method($($pname,)*),
      S::Python => Python.$method($($pname,)*),
      S::Rust => Rust.$method($($pname,)*),
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

/// Guess which programming language a file is written in
/// Adapt from `<https://github.com/Wilfred/difftastic/blob/master/src/parse/guess_language.rs>`
fn from_extension(path: &Path) -> Option<SupportLang> {
  use SupportLang::*;
  match path.extension()?.to_str()? {
    "c" | "h" => Some(C),
    "cc" | "hpp" | "cpp" | "c++" | "hh" | "cxx" | "cu" | "ino" => Some(Cpp),
    "cs" => Some(CSharp),
    "css" | "scss" => Some(Css),
    "dart" => Some(Dart),
    "go" => Some(Go),
    "html" | "htm" | "xhtml" => Some(Html),
    "java" => Some(Java),
    "cjs" | "js" | "mjs" | "jsx" => Some(JavaScript),
    "kt" | "ktm" | "kts" => Some(Kotlin),
    "lua" => Some(Lua),
    "py" | "py3" | "pyi" | "bzl" => Some(Python),
    "rs" => Some(Rust),
    "swift" => Some(Swift),
    "thrift" => Some(Thrift),
    "ts" | "cts" | "mts" => Some(TypeScript),
    "tsx" => Some(Tsx),
    _ => None,
  }
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
    L::Kotlin => builder.select("kotlin"),
    L::Lua => builder.select("lua"),
    L::Python => builder.select("py"),
    L::Rust => builder.select("rust"),
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
