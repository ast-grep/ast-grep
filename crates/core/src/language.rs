use crate::meta_var::{extract_meta_var, MetaVariable};
use crate::pattern::Pattern;
use crate::AstGrep;
pub use tree_sitter::Language as TSLanguage;
use tree_sitter_c::language as language_c;
use tree_sitter_go::language as language_go;
use tree_sitter_html::language as language_html;
use tree_sitter_javascript::language as language_javascript;
use tree_sitter_kotlin::language as language_kotlin;
use tree_sitter_lua::language as language_lua;
use tree_sitter_python::language as language_python;
use tree_sitter_rust::language as language_rust;
use tree_sitter_swift::language as language_swift;
use tree_sitter_typescript::{language_tsx, language_typescript};

pub trait Language: Copy + Clone {
    /// Create an [`AstGrep`] instance for the language
    fn new<S: AsRef<str>>(&self, source: S) -> AstGrep<Self> {
        AstGrep::new(source, *self)
    }

    /// tree sitter language to parse the source
    fn get_ts_language(&self) -> TSLanguage;
    /// ignore trivial tokens in language matching
    fn skippable_kind_ids(&self) -> &'static [u16] {
        &[]
    }

    /// Configure meta variable special character
    /// By default $ is the metavar char, but in PHP it is #
    #[inline]
    fn meta_var_char(&self) -> char {
        '$'
    }
    /// extract MetaVariable from a given source string
    fn extract_meta_var(&self, source: &str) -> Option<MetaVariable> {
        extract_meta_var(source, self.meta_var_char())
    }
    /// normalize query before matching
    /// e.g. remove expression_statement, or prefer parsing {} to object over block
    fn build_pattern(&self, query: &str) -> Pattern<Self> {
        Pattern::new(query, *self)
    }
}

macro_rules! impl_lang {
    ($lang: ident, $func: ident) => {
        #[derive(Clone, Copy)]
        pub struct $lang;
        impl Language for $lang {
            fn get_ts_language(&self) -> TSLanguage {
                $func()
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
impl_lang!(Rust, language_rust);
impl_lang!(Swift, language_swift);
impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);
