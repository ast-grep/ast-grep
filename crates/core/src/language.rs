use crate::meta_var::{extract_meta_var, MetaVariable};
use crate::pattern::Pattern;
use crate::AstGrep;
pub use tree_sitter::Language as TSLanguage;

pub trait Language: Clone {
    /// Create an [`AstGrep`] instance for the language
    fn ast_grep<S: AsRef<str>>(&self, source: S) -> AstGrep<Self> {
        AstGrep::new(source, self.clone())
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
        Pattern::new(query, self.clone())
    }
}

impl Language for TSLanguage {
    fn get_ts_language(&self) -> TSLanguage {
        self.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[derive(Clone)]
    pub struct Tsx;
    impl Language for Tsx {
        fn get_ts_language(&self) -> TSLanguage {
            tree_sitter_typescript::language_tsx().into()
        }
    }
}

#[cfg(test)]
pub use test::*;
