mod rust;
use ignore::types::{Types, TypesBuilder};
use std::borrow::Cow;
use std::path::Path;

use tree_sitter_c::language as language_c;
use tree_sitter_go::language as language_go;
use tree_sitter_html::language as language_html;
use tree_sitter_javascript::language as language_javascript;
use tree_sitter_kotlin::language as language_kotlin;
use tree_sitter_lua::language as language_lua;
use tree_sitter_python::language as language_python;
use tree_sitter_swift::language as language_swift;
use tree_sitter_typescript::{language_tsx, language_typescript};

pub use rust::Rust;

macro_rules! impl_lang {
    ($lang: ident, $func: ident) => {
        #[derive(Clone, Copy)]
        pub struct $lang;
        impl Language for $lang {
            fn get_ts_language(&self) -> TSLanguage {
                $func().into()
            }
        }
    };
}

impl_lang!(C, language_c);
impl_lang!(Go, language_go);
impl_lang!(Html, language_html);
impl_lang!(JavaScript, language_javascript);
impl_lang!(Kotlin, language_kotlin);
impl_lang!(Lua, language_lua);
impl_lang!(Python, language_python);
impl_lang!(Swift, language_swift);
impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);

use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::MetaVariable;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// represents a dynamic language
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportLang {
    C,
    Go,
    Html,
    JavaScript,
    Kotlin,
    Lua,
    Python,
    Rust,
    Swift,
    Tsx,
    TypeScript,
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
            "go" | "golang" => Ok(Go),
            "html" => Ok(Html),
            "js" | "jsx" => Ok(JavaScript),
            "kt" | "ktm" | "kts" => Ok(Kotlin),
            "lua" => Ok(Lua),
            "py" | "python" => Ok(Python),
            "rs" | "rust" => Ok(Rust),
            "swift" => Ok(Swift),
            "ts" => Ok(TypeScript),
            "tsx" => Ok(Tsx),
            _ => Err(SupportLangErr::LanguageNotSupported(s.to_string())),
        }
    }
}

macro_rules! impl_lang_method {
    ($method: ident, $return_type: ty) => {
        #[inline]
        fn $method(&self) -> $return_type {
            use SupportLang as S;
            match self {
                S::C => C.$method(),
                S::Go => Go.$method(),
                S::Html => Html.$method(),
                S::JavaScript => JavaScript.$method(),
                S::Kotlin => Kotlin.$method(),
                S::Lua => Lua.$method(),
                S::Python => Python.$method(),
                S::Rust => Rust.$method(),
                S::Swift => Swift.$method(),
                S::Tsx => Tsx.$method(),
                S::TypeScript => TypeScript.$method(),
            }
        }
    };
}

// TODO: optimize this using macro
impl Language for SupportLang {
    impl_lang_method!(get_ts_language, TSLanguage);
    impl_lang_method!(meta_var_char, char);
    impl_lang_method!(expando_char, char);

    fn extract_meta_var(&self, source: &str) -> Option<MetaVariable> {
        use SupportLang as S;
        match self {
            S::C => C.extract_meta_var(source),
            S::Go => Go.extract_meta_var(source),
            S::Html => Html.extract_meta_var(source),
            S::JavaScript => JavaScript.extract_meta_var(source),
            S::Kotlin => Kotlin.extract_meta_var(source),
            S::Lua => Lua.extract_meta_var(source),
            S::Python => Python.extract_meta_var(source),
            S::Rust => Rust.extract_meta_var(source),
            S::Swift => Swift.extract_meta_var(source),
            S::Tsx => Tsx.extract_meta_var(source),
            S::TypeScript => TypeScript.extract_meta_var(source),
        }
    }

    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        use SupportLang as S;
        match self {
            S::C => C.pre_process_pattern(query),
            S::Go => Go.pre_process_pattern(query),
            S::Html => Html.pre_process_pattern(query),
            S::JavaScript => JavaScript.pre_process_pattern(query),
            S::Kotlin => Kotlin.pre_process_pattern(query),
            S::Lua => Lua.pre_process_pattern(query),
            S::Python => Python.pre_process_pattern(query),
            S::Rust => Rust.pre_process_pattern(query),
            S::Swift => Swift.pre_process_pattern(query),
            S::Tsx => Tsx.pre_process_pattern(query),
            S::TypeScript => TypeScript.pre_process_pattern(query),
        }
    }
}

/// Guess which programming language a file is written in
/// Adapt from https://github.com/Wilfred/difftastic/blob/master/src/parse/guess_language.rs
pub fn from_extension(path: &Path) -> Option<SupportLang> {
    use SupportLang::*;
    match path.extension()?.to_str()? {
        "c" | "h" => Some(C),
        "go" => Some(Go),
        "html" | "htm" | "xhtml" => Some(Html),
        "cjs" | "js" | "mjs" | "jsx" => Some(JavaScript),
        "kt" | "ktm" | "kts" => Some(Kotlin),
        "lua" => Some(Lua),
        "py" | "py3" | "pyi" | "bzl" => Some(Python),
        "rs" => Some(Rust),
        "swift" => Some(Swift),
        "ts" => Some(TypeScript),
        "tsx" => Some(Tsx),
        _ => None,
    }
}

pub fn file_types(lang: &SupportLang) -> Types {
    use SupportLang as L;
    let mut builder = TypesBuilder::new();
    builder.add_defaults();
    let builder = match lang {
        L::C => builder.select("c"),
        L::Go => builder.select("go"),
        L::Html => builder.select("html"),
        L::JavaScript => {
            builder.add("myjs", "*.js").unwrap();
            builder.add("myjs", "*.cjs").unwrap();
            builder.add("myjs", "*.jsx").unwrap();
            builder.add("myjs", "*.mjs").unwrap();
            builder.select("myjs")
        }
        L::Kotlin => builder.select("kotlin"),
        L::Lua => builder.select("lua"),
        L::Python => builder.select("py"),
        L::Rust => builder.select("rust"),
        L::Swift => builder.select("swift"),
        L::Tsx => {
            builder.add("mytsx", "*.tsx").unwrap();
            builder.select("mytsx")
        }
        L::TypeScript => {
            builder.add("myts", "*.ts").unwrap();
            builder.add("myts", "*.cts").unwrap();
            builder.add("myts", "*.mts").unwrap();
            builder.select("myts")
        }
    };
    builder.build().unwrap()
}

pub fn config_file_type() -> Types {
    let mut builder = TypesBuilder::new();
    builder.add("yml", "*.yml").unwrap();
    builder.add("yml", "*.yaml").unwrap();
    builder.select("yml");
    builder.build().unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_guess_by_extension() {
        let path = Path::new("foo.rs");
        assert_eq!(from_extension(path), Some(SupportLang::Rust));
    }
}
