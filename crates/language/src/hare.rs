#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_non_match_lang, test_replace_lang};

#[test]
fn test_hare_pattern() {
  assert!(
    !Hare
      .ast_grep("const x = 42;")
      .root()
      .get_inner_node()
      .has_error()
  );
  test_match_lang("const $NAME = $VALUE;", "const x = 42;", Hare);
  test_match_lang(
    "fn $NAME($$$ARGS) int = $BODY;",
    "fn add(a: int, b: int) int = a + b;",
    Hare,
  );
  test_non_match_lang("const $NAME = 42;", "const x = 43;", Hare);
}

#[test]
fn test_hare_contextual_call() {
  let source = "export fn main() void = { const x = add(1, 2); fmt::println(x)!; };";
  let ast = Hare.ast_grep(source);
  assert!(!ast.root().get_inner_node().has_error());
  let pattern = Pattern::contextual(
    "export fn _probe() void = { $FN($$$ARGS); };",
    "call_expression",
    Hare,
  )
  .unwrap();
  let matches: Vec<_> = ast.root().find_all(pattern).collect();
  assert_eq!(matches.len(), 2);
  assert_eq!(matches[0].text(), "add(1, 2)");
  assert_eq!(matches[0].get_env().get_match("FN").unwrap().text(), "add");
  assert_eq!(matches[0].get_env().get_multiple_matches("ARGS").len(), 3);
  assert_eq!(matches[1].text(), "fmt::println(x)");
}

#[test]
fn test_hare_replace() {
  assert_eq!(
    test_replace_lang(
      "const x = 42;",
      "const $NAME = $VALUE;",
      "const $NAME = 0;",
      Hare
    ),
    "const x = 0;",
  );
  let mut ast = Hare.ast_grep("export fn main() void = { const x = add(1, 2); };");
  let pattern = Pattern::contextual(
    "export fn _probe() void = { const $NAME = $VALUE; };",
    "const_declaration",
    Hare,
  )
  .unwrap();
  assert!(ast.replace(pattern, "const $NAME = 0;").unwrap());
  assert_eq!(ast.generate(), "export fn main() void = { const x = 0; };");
}

#[test]
fn test_hare_registration() {
  assert_eq!("hare".parse::<SupportLang>().unwrap(), SupportLang::Hare);
  assert_eq!(SupportLang::from_path("main.ha"), Some(SupportLang::Hare));
  assert_eq!(SupportLang::Hare.expando_char(), '_');
}
