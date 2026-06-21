use ast_grep_language::SupportLang;
use ast_grep_outline::{
  DEFAULT_OUTLINE_RULES, combined_extractor::CombinedExtractors, extractor::parse_outline_rules,
};

#[test]
fn bundled_outline_rules_compile_for_every_language() {
  const OUTLINE_LANGUAGES: &[SupportLang] = &[
    SupportLang::Rust,
    SupportLang::TypeScript,
    SupportLang::Tsx,
    SupportLang::JavaScript,
    SupportLang::Python,
    SupportLang::Go,
    SupportLang::Kotlin,
    SupportLang::Java,
    SupportLang::Swift,
  ];

  for language in OUTLINE_LANGUAGES {
    let rules = parse_outline_rules::<SupportLang>(DEFAULT_OUTLINE_RULES)
      .expect("builtin outline rules should deserialize")
      .into_iter()
      .filter(|rule| rule.common().language == *language)
      .collect::<Vec<_>>();

    assert!(!rules.is_empty(), "{language:?} should have outline rules");
    CombinedExtractors::try_from(rules, &Default::default())
      .unwrap_or_else(|_| panic!("{language:?} outline rules should compile"));
  }
}
