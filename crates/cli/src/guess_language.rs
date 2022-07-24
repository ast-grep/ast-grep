//! Guess which programming language a file is written in
//! Adapt from https://github.com/Wilfred/difftastic/blob/master/src/parse/guess_language.rs
use ast_grep_core::language::{self, Language, TSLanguage};
use ast_grep_core::MetaVariable;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;
use ignore::types::{Types, TypesBuilder};

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

impl SupportLang {
    pub fn file_types(&self) -> Types {
        use SupportLang::*;
        let mut builder = TypesBuilder::new();
        builder.add_defaults();
        let builder = match self {
            C => builder.select("c"),
            Go => builder.select("go"),
            Html => builder.select("html"),
            JavaScript => {
                builder.add("myjs", "*.js").unwrap();
                builder.add("myjs", "*.jsx").unwrap();
                builder.add("myjs", "*.mjs").unwrap();
                builder.select("myjs")
            }
            Kotlin => builder.select("kotlin"),
            Lua => builder.select("lua"),
            Python => builder.select("py"),
            Rust => builder.select("rust"),
            Swift => builder.select("swift"),
            Tsx => {
                builder.add("mytsx", "*.tsx").unwrap();
                builder.select("mytsx")
            }
            TypeScript => {
                builder.add("myts", "*.ts").unwrap();
                builder.select("myts")
            }
        };
        builder.build().unwrap()
    }

}

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
            use SupportLang::*;
            match self {
                C => language::C.$method(),
                Go => language::Go.$method(),
                Html => language::Html.$method(),
                JavaScript => language::JavaScript.$method(),
                Kotlin => language::Kotlin.$method(),
                Lua => language::Lua.$method(),
                Python => language::Python.$method(),
                Rust => language::Rust.$method(),
                Swift => language::Swift.$method(),
                Tsx => language::Tsx.$method(),
                TypeScript => language::TypeScript.$method(),
            }
        }
    }
}
impl Language for SupportLang {
    impl_lang_method!(get_ts_language, TSLanguage);
    impl_lang_method!(meta_var_char, char);

    fn extract_meta_var(&self, source: &str) -> Option<MetaVariable> {
        use SupportLang::*;
        match self {
            C => language::C.extract_meta_var(source),
            Go => language::Go.extract_meta_var(source),
            Html => language::Html.extract_meta_var(source),
            JavaScript => language::JavaScript.extract_meta_var(source),
            Kotlin => language::Kotlin.extract_meta_var(source),
            Lua => language::Lua.extract_meta_var(source),
            Python => language::Python.extract_meta_var(source),
            Rust => language::Rust.extract_meta_var(source),
            Swift => language::Swift.extract_meta_var(source),
            Tsx => language::Tsx.extract_meta_var(source),
            TypeScript => language::TypeScript.extract_meta_var(source),
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
