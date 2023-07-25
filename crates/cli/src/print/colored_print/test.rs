#![cfg(test)]

use super::*;
use ast_grep_config::{from_yaml_string, GlobalRules};
use ast_grep_core::replacer::Fixer;
use ast_grep_language::{Language, SupportLang};
use codespan_reporting::term::termcolor::Buffer;

fn make_test_printer() -> ColoredPrinter<Buffer> {
  ColoredPrinter::new(Buffer::no_color()).color(ColorChoice::Never)
}
fn get_text(printer: &ColoredPrinter<Buffer>) -> String {
  let buffer = printer.writer.lock().expect("should work");
  let bytes = buffer.as_slice();
  std::str::from_utf8(bytes)
    .expect("buffer should be valid utf8")
    .to_owned()
}

#[test]
fn test_emtpy_printer() {
  let printer = make_test_printer();
  assert_eq!(get_text(&printer), "");
}

// source, pattern, debug note
type Case<'a> = (&'a str, &'a str, &'a str);

const MATCHES_CASES: &[Case] = &[
  ("let a = 123", "a", "Simple match"),
  ("Some(1), Some(2), Some(3)", "Some", "Same line match"),
  (
    "Some(1), Some(2)\nSome(3), Some(4)",
    "Some",
    "Multiple line match",
  ),
  (
    "import a from 'b';import a from 'b';",
    "import a from 'b';",
    "immediate following but not overlapping",
  ),
  ("Some(Some(123))", "Some($A)", "overlapping"),
];
#[test]
fn test_print_matches() {
  for &(source, pattern, note) in MATCHES_CASES {
    // heading is required for CI
    let printer = make_test_printer().heading(Heading::Always);
    let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
    let matches = grep.root().find_all(pattern);
    printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
    let expected: String = source
      .lines()
      .enumerate()
      .map(|(i, l)| format!("{}│{l}\n", i + 1))
      .collect();
    // append heading to expected
    let output = format!("test.tsx\n{expected}\n");
    assert_eq!(get_text(&printer), output, "{note}");
  }
}

#[test]
fn test_print_matches_without_heading() {
  for &(source, pattern, note) in MATCHES_CASES {
    let printer = make_test_printer().heading(Heading::Never);
    let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
    let matches = grep.root().find_all(pattern);
    printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
    // append heading to expected
    let output: String = source
      .lines()
      .enumerate()
      .map(|(i, e)| format!("test.tsx:{}:{e}\n", i + 1))
      .collect();
    assert_eq!(get_text(&printer), output, "{note}");
  }
}

#[test]
fn test_print_rules() {
  let globals = GlobalRules::default();
  for &(source, pattern, note) in MATCHES_CASES {
    let printer = make_test_printer()
      .heading(Heading::Never)
      .style(ReportStyle::Short);
    let grep = SgLang::from(SupportLang::TypeScript).ast_grep(source);
    let source = source.to_string();
    let file = SimpleFile::new(Cow::Borrowed("test.tsx"), &source);
    let rule = from_yaml_string(
      &format!(
        r"
id: test-id
message: test rule
severity: info
language: TypeScript
rule:
  pattern: {pattern}"
      ),
      &globals,
    )
    .expect("should parse")
    .pop()
    .unwrap();
    let matcher = rule.get_matcher(&globals).expect("should parse");
    let matches = grep.root().find_all(&matcher);
    printer.print_rule(matches, file, &rule).expect("test only");
    let text = get_text(&printer);
    assert!(text.contains("test.tsx"), "{note}");
    assert!(text.contains("note[test-id]"), "{note}");
    assert!(text.contains("test rule"), "{note}");
  }
}

// source, pattern, rewrite, debug note
type DiffCase<'a> = (&'a str, &'a str, &'a str, &'a str);

const DIFF_CASES: &[DiffCase] = &[
  ("let a = 123", "a", "b", "Simple match"),
  (
    "Some(1), Some(2), Some(3)",
    "Some",
    "Any",
    "Same line match",
  ),
  (
    "Some(1), Some(2)\nSome(3), Some(4)",
    "Some",
    "Any",
    "Multiple line match",
  ),
  (
    "import a from 'b';import a from 'b';",
    "import a from 'b';",
    "",
    "immediate following but not overlapping",
  ),
  (
    "\n\ntest",
    "test",
    "rest",
    // https://github.com/ast-grep/ast-grep/issues/517
    "leading empty space",
  ),
];

#[test]
fn test_print_diffs() {
  for &(source, pattern, rewrite, note) in DIFF_CASES {
    // heading is required for CI
    let printer = make_test_printer().heading(Heading::Always);
    let lang = SgLang::from(SupportLang::Tsx);
    let fixer = Fixer::try_new(rewrite, &lang).expect("should work");
    let grep = lang.ast_grep(source);
    let matches = grep.root().find_all(pattern);
    let diffs = matches.map(|n| Diff::generate(n, &pattern, &fixer));
    printer.print_diffs(diffs, "test.tsx".as_ref()).unwrap();
    assert!(get_text(&printer).contains(rewrite), "{note}");
  }
}

fn test_overlap_print_impl(heading: Heading) {
  let src = "
    Some(1)
    // empty
    Some(2)
  ";
  let printer = make_test_printer().heading(heading).context((1, 1));
  let lang = SgLang::from(SupportLang::Tsx);
  let grep = lang.ast_grep(src);
  let matches = grep.root().find_all("Some($A)");
  printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
  let text = get_text(&printer);
  // Overlapped match should only print once.
  assert_eq!(text.matches("Some(1)").count(), 1);
  assert_eq!(text.matches("empty").count(), 1);
  assert_eq!(text.matches("Some(2)").count(), 1);
}

#[test]
fn test_overlap_print() {
  // test_overlap_print_impl(Heading::Always);
  test_overlap_print_impl(Heading::Never);
  // test_overlap_print_impl(Heading::Auto);
}

fn test_non_overlap_print_impl(heading: Heading) {
  let src = "
    Some(1)
    // empty
    Some(2)
  ";
  let printer = make_test_printer().heading(heading);
  let lang = SgLang::from(SupportLang::Tsx);
  let grep = lang.ast_grep(src);
  let matches = grep.root().find_all("Some($A)");
  printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
  let text = get_text(&printer);
  assert_eq!(text.matches("Some(1)").count(), 1);
  assert!(!text.contains("empty"));
  assert_eq!(text.matches("Some(2)").count(), 1);
}

#[test]
fn test_non_overlap_print() {
  test_non_overlap_print_impl(Heading::Always);
  test_non_overlap_print_impl(Heading::Never);
  test_non_overlap_print_impl(Heading::Auto);
}

#[test]
fn test_print_rule_diffs() {
  let globals = GlobalRules::default();
  for &(source, pattern, rewrite, note) in DIFF_CASES {
    let printer = make_test_printer()
      .heading(Heading::Never)
      .style(ReportStyle::Short);
    let grep = SgLang::from(SupportLang::TypeScript).ast_grep(source);
    let rule = from_yaml_string(
      &format!(
        r"
id: test-id
message: test rule
severity: info
language: TypeScript
rule:
  pattern: {pattern}
fix: '{rewrite}'"
      ),
      &globals,
    )
    .expect("should parse")
    .pop()
    .unwrap();
    let matcher = rule.get_matcher(&globals).expect("should parse");
    let fixer = rule.fixer.as_ref().expect("should have fixer");
    let matches = grep.root().find_all(&matcher);
    let diffs = matches.map(|n| Diff::generate(n, &pattern, fixer));
    printer
      .print_rule_diffs(diffs, Path::new("test.tsx"), &rule)
      .expect("test only");
    let text = get_text(&printer);
    assert!(text.contains("test.tsx"), "{note}");
    assert!(text.contains("note[test-id]"), "{note}");
    assert!(text.contains(rewrite), "{note}");
  }
}

#[test]
fn test_before_after() {
  let src = "
    // b 3
    // b 2
    // b 1
    Some(match)
    // a 1
    // a 2
    // a 3
  ";
  for b in 0..3 {
    for a in 0..3 {
      let printer = make_test_printer().context((b, a));
      let lang = SgLang::from(SupportLang::Tsx);
      let grep = lang.ast_grep(src);
      let matches = grep.root().find_all("Some($A)");
      printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
      let text = get_text(&printer);
      // Overlapped match should only print once.
      assert!(text.contains("Some(match)"));
      for i in 1..3 {
        let contains_before = text.contains(&format!("b {i}"));
        let b_in_bound = i <= b;
        let contains_after = text.contains(&format!("a {i}"));
        let a_in_bound = i <= a;
        // text occurrence should be the same as inbound check
        assert_eq!(contains_before, b_in_bound);
        assert_eq!(contains_after, a_in_bound);
      }
    }
  }
}
