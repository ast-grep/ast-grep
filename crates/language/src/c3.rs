#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_non_match_lang, test_replace_lang};

// C3 types have different identifier casing, so type positions use literal types.
#[test]
fn test_c3_pattern() {
  test_match_lang(
    "fn int $NAME(int $A, int $B) { return $A + $B; }",
    "fn int add(int a, int b) { return a + b; }",
    C3,
  );
  test_non_match_lang(
    "fn int $NAME(int $A, int $B) { return $A + $A; }",
    "fn int add(int a, int b) { return a + b; }",
    C3,
  );
}

#[test]
fn test_c3_contextual_call() {
  let source = "module test; fn void main() { int x = add(1, 2); io::printn(x); }";
  let ast = C3.ast_grep(source);
  assert!(!ast.root().get_inner_node().has_error());
  let pattern = Pattern::contextual("fn void _probe() { $FN($$$ARGS); }", "call_expr", C3).unwrap();
  let matches: Vec<_> = ast.root().find_all(pattern).collect();
  assert_eq!(matches.len(), 2);
  assert_eq!(matches[0].text(), "add(1, 2)");
  assert_eq!(matches[0].get_env().get_match("FN").unwrap().text(), "add");
  assert_eq!(matches[0].get_env().get_multiple_matches("ARGS").len(), 3);
  assert_eq!(matches[1].text(), "io::printn(x)");
}

#[test]
fn test_c3_replace() {
  assert_eq!(
    test_replace_lang(
      "fn int add(int a, int b) { return a + b; }",
      "fn int $NAME(int $A, int $B) { return $A + $B; }",
      "fn int $NAME(int $A, int $B) { return $B + $A; }",
      C3,
    ),
    "fn int add(int a, int b) { return b + a; }",
  );
}

#[test]
fn test_c3_registration() {
  assert_eq!("c3".parse::<SupportLang>().unwrap(), SupportLang::C3);
  for path in ["main.c3", "header.c3i", "template.c3t"] {
    assert_eq!(SupportLang::from_path(path), Some(SupportLang::C3));
  }
  assert_eq!(SupportLang::C3.expando_char(), 'a');
}
