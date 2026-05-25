#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Markdown);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Markdown);
}

#[test]
fn test_markdown_heading() {
  test_match("# $TITLE\n", "# Hello\n");
  test_match("## $TITLE\n", "## Hello\n");
  test_non_match("# $TITLE", "paragraph");
}

#[test]
fn test_markdown_list() {
  test_match("- $ITEM", "- item");
  test_match("- [ ] $ITEM", "- [ ] item");
  test_match("- [x] $ITEM", "- [x] item");
}

#[test]
fn test_markdown_fenced_code_block() {
  test_match(
    "```rust\nfn main() {}\n```\n",
    "```rust\nfn main() {}\n```\n",
  );
}
