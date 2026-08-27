use ast_grep_language::SupportLang;

#[allow(dead_code)]
mod common;

#[test]
fn markdown_rules_extract_atx_and_setext_headings() {
  const RULES: &str = include_str!("../src/default_rules/markdown.yml");
  common::assert_outline_snapshot(
    SupportLang::Markdown,
    RULES,
    include_str!("fixtures/markdown_headings.md"),
    r#"
- Module item exported First
- Module item exported Second
- Module item exported Third
- Module item exported Fourth
- Module item exported Fifth
- Module item exported Sixth
- Module item exported Setext One
- Module item exported Setext Two
"#,
  );
}
