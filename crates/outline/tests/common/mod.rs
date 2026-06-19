use ast_grep_language::SupportLang;
use ast_grep_outline::{
  DEFAULT_OUTLINE_RULES, combined_extractor::CombinedExtractors, extractor::parse_outline_rules,
};

pub fn combined() -> CombinedExtractors<SupportLang> {
  let rules = parse_outline_rules::<SupportLang>(DEFAULT_OUTLINE_RULES)
    .expect("builtin outline rules should deserialize");
  CombinedExtractors::try_from(rules, &Default::default()).expect("builtin rules should compile")
}
