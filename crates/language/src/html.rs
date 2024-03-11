#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Html);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Html);
}

#[test]
fn test_html_match() {
  test_match("<input>", "<input>");
  test_match("<$TAG>", "<input>");
  test_match("<$TAG class='foo'>$$$</$TAG>", "<div class='foo'></div>");
  test_match("<div>$$$</div>", "<div>123</div>");
  test_non_match("<$TAG class='foo'>$$$</$TAG>", "<div></div>");
  test_non_match("<div>$$$</div>", "<div class='foo'>123</div>");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Html)
}

#[test]
fn test_html_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    r#"<div class='foo'>bar</div>"#,
    r#"<$TAG class='foo'>$$$B</$TAG>"#,
    r#"<$TAG class='$$$B'>foo</$TAG>"#,
  )?;
  assert_eq!(ret, r#"<div class='bar'>foo</div>"#);
  Ok(())
}
