pub trait Language {
    fn skippable_kind_ids() -> &'static [u16];
    fn gen_meta_varaible(meta_var_source: &str) -> String;
}
