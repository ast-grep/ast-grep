#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Lua);
}

#[test]
fn test_lua_pattern() {
  test_match("s = $S", "s = 'string'");
  test_match("print($S)", "print('Hello World')");
  test_match("a = io.$METHOD($S)", "a = io.read('*number')");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Lua)
}

#[test]
fn test_lua_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    r#"function fact (n)
      if n == 0 then
        return 1
      else
        return n * fact(n-1)
      end
    end"#,
    "function $FUNC($ARG) $$$ end",
    "$FUNC = function ($ARG) return 1 end",
  )?;
  assert_eq!(ret, "fact = function (n) return 1 end");
  Ok(())
}
