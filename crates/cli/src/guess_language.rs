//! Guess which programming language a file is written in
//! Adapt from https://github.com/Wilfred/difftastic/blob/master/src/parse/guess_language.rs
use ast_grep_core::language::{self, Language, TSLanguage};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;

/// represents a dynamic language
#[derive(Clone, Copy, Debug, PartialEq)]
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

pub fn from_extension(path: &Path) -> Option<SupportLang> {
    use SupportLang::*;
    match path.extension()?.to_str()? {
        "c" => Some(C),
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

impl Language for SupportLang {
    fn get_ts_language(&self) -> TSLanguage {
        use SupportLang::*;
        match self {
            C => language::C.get_ts_language(),
            Go => language::Go.get_ts_language(),
            Html => language::Html.get_ts_language(),
            JavaScript => language::JavaScript.get_ts_language(),
            Kotlin => language::Kotlin.get_ts_language(),
            Lua => language::Lua.get_ts_language(),
            Python => language::Python.get_ts_language(),
            Rust => language::Rust.get_ts_language(),
            Swift => language::Swift.get_ts_language(),
            Tsx => language::Tsx.get_ts_language(),
            TypeScript => language::TypeScript.get_ts_language(),
        }
    }
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
