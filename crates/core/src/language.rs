use crate::AstGrep;
use crate::pattern::Pattern;
use crate::meta_var::{MetaVariable, extract_meta_var};
pub use tree_sitter::Language as TSLanguage;
use tree_sitter_c::language as language_c;
use tree_sitter_typescript::{language_tsx, language_typescript};

pub trait Language: Sized {
    /// Create an [`AstGrep`] instance for the language
    fn new(source: &str) -> AstGrep<Self> {
        AstGrep::new(source)
    }

    /// tree sitter language to parse the source
    fn get_ts_language() -> TSLanguage;
    /// ignore trivial tokens in language matching
    fn skippable_kind_ids() -> &'static [u16] {
        &[]
    }

    /// Configure meta variable special character
    /// By default $ is the metavar char, but in PHP it is #
    #[inline]
    fn meta_var_char() -> char {
        '$'
    }
    /// extract MetaVariable from a given source string
    fn extract_meta_var(source: &str) -> Option<MetaVariable> {
        extract_meta_var(source, Self::meta_var_char())
    }
    /// normalize query before matching
    /// e.g. remove expression_statement, or prefer parsing {} to object over block
    fn build_pattern(query: &str) -> Pattern {
        Pattern::new(query, Self::get_ts_language())
    }
}

macro_rules! impl_lang {
    ($lang: ident, $func: ident) => {
        pub struct $lang;
        impl Language for $lang {
            fn get_ts_language() -> TSLanguage {
                $func()
            }
        }
    }
}

impl_lang!(Tsx, language_tsx);
impl_lang!(TypeScript, language_typescript);
impl_lang!(Clang, language_c);
