use crate::pattern::Pattern;

pub trait Language {
    /// tree sitter language to parse the source
    fn get_ts_language() -> tree_sitter::Language;
    /// ignore trivial tokens in language matching
    fn skippable_kind_ids() -> &'static [u16];

    /// Configure meta variable special character
    /// By default $ is the metavar char, but in PHP it is #
    fn meta_var_char() -> char {
        '$'
    }
    /// normalize query before matching
    /// e.g. remove expression_statement, or prefer parsing {} to object over block
    fn build_query(query: &str) -> Pattern;
}
