pub trait Language {
    /// ignore trivial tokens in language matching
    fn skippable_kind_ids() -> &'static [u16];
    fn gen_meta_varaible(meta_var_source: &str) -> String;
    /// normalize query before matching
    /// e.g. remove expression_statement, or prefer parsing {} to object over block
    fn build_query(query: &str) -> String;
}
