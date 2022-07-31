//! Guess which programming language a file is written in
//! Adapt from https://github.com/Wilfred/difftastic/blob/master/src/parse/guess_language.rs
pub use ast_grep_config::SupportLang;
use ignore::types::{Types, TypesBuilder};
use std::path::Path;

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


#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_guess_by_extension() {
        let path = Path::new("foo.rs");
        assert_eq!(from_extension(path), Some(SupportLang::Rust));
    }
}
