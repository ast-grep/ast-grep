use crate::parsers::language_python;
use ast_grep_core::language::{Language, TSLanguage};
use std::borrow::Cow;

// impl_lang!(Python, language_python);
#[derive(Clone, Copy)]
pub struct Python;
impl Language for Python {
  fn get_ts_language(&self) -> TSLanguage {
    language_python()
  }
  // we can use any char in unicode range [:XID_Start:]
  // https://docs.python.org/3/reference/lexical_analysis.html#identifiers
  // see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
  fn expando_char(&self) -> char {
    'µ'
  }
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    // use stack buffer to reduce allocation
    let mut buf = [0; 4];
    let expando = self.expando_char().encode_utf8(&mut buf);
    // TODO: use more precise replacement
    let replaced = query.replace(self.meta_var_char(), expando);
    Cow::Owned(replaced)
  }
}

#[cfg(test)]
mod test {
  use ast_grep_core::source::TSParseError;

  use super::*;

  fn test_match(query: &str, source: &str) {
    use crate::test::test_match_lang;
    test_match_lang(query, source, Python);
  }

  fn test_non_match(query: &str, source: &str) {
    use crate::test::test_non_match_lang;
    test_non_match_lang(query, source, Python);
  }

  #[test]
  fn test_python_str() {
    test_match("print($A)", "print(123)");
    test_match("print('123')", "print('123')");
    test_non_match("print('123')", "print('456')");
    test_non_match("'123'", "'456'");
  }

  #[test]
  fn test_python_pattern() {
    test_match("$A = 0", "a = 0");
    // A test case from https://peps.python.org/pep-0636/#appendix-a-quick-intro
    test_match(
      r#"
match $A:
    case $B:
        $C
    case [$D(0, 0)]:
        $E
    case [$D($F, $G)]:
        $H
    case [$D(0, $I), $D(0, $J)]:
        $K
    case _:
        $L
"#,
      r#"
match points:
    case []:
        print("No points")
    case [Point(0, 0)]:
        print("The origin")
    case [Point(x, y)]:
        print(f"Single point {x}, {y}")
    case [Point(0, y1), Point(0, y2)]:
        print(f"Two on the Y axis at {y1}, {y2}")
    case _:
        print("Something else")
"#,
    );
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
    use crate::test::test_replace_lang;
    test_replace_lang(src, pattern, replacer, Python)
  }

  #[test]
  fn test_python_replace() -> Result<(), TSParseError> {
    let ret = test_replace(
      r#"
if flag:
    a = value_pos
else:
    a = value_neg"#,
      r#"
if $FLAG:
    $VAR = $POS
else:
    $VAR = $NEG
"#,
      "$VAR = $POS if $FLAG else $NEG",
    )?;
    assert_eq!(ret, "\na = value_pos if flag else value_neg");

    let ret = test_replace(
      r#"
try:
    f = open(file_path, "r")
    file_content = f.read()
except:
    pass
finally:
    f.close()"#,
      r#"
try:
    $A = open($B, $C)
    $D = $A.read()
except:
    pass
finally:
    $A.close()"#,
      r#"
with open($B, $C) as $A:
    $D = $A.open()"#,
    )?;
    assert_eq!(
      ret,
      r#"

with open(file_path, "r") as f:
    file_content = f.open()"#
    );
    Ok(())
  }
}
