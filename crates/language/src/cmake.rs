#![cfg(test)]
use ast_grep_core::source::TSParseError;

use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, Cmake);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, Cmake);
}

#[test]
fn test_cmake_str() {
  test_match("123", "123");
  test_match("${PROJECT_NAME}", "${PROJECT_NAME}");
  test_match("message($A)", "message(\"One message\")");
  test_match("message(\"One message\")", "message(\"One message\")");
  test_match(
    "message(\"$MESSAGE\")",
    "message(\"Target does not exist yet: ${target_name}\")",
  );
  test_non_match("message(\"One message\")", "message(\"Two message\")");
  test_non_match("'123'", "'456'");
}

#[test]
fn test_cmake_function_declaration() {
  test_match("function($A)", "function(func_name)");
  test_match("function($A $B $C)", "function(func_name target out_var)");
  test_non_match("function($A $B $C $D)", "function(func_name target)");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> Result<String, TSParseError> {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, Cmake)
}

#[test]
fn test_cmake_replace() -> Result<(), TSParseError> {
  let ret = test_replace(
    r#"
cmake_minimum_required(VERSION 3.16)
project(myapp LANGUAGES CXX VERSION 1.0.0 DESCRIPTION "My CMake Project")
add_executable(myapp main.cpp)
"#,
    r#"project($$$LEFT_OPTIONS VERSION $VERSION $$$RIGHT_OPTIONS)"#,
    r#"project($$$LEFT_OPTIONS VERSION 2.0.0 $$$RIGHT_OPTIONS)"#,
  )?;
  assert_eq!(
    ret,
    r#"
cmake_minimum_required(VERSION 3.16)
project(myapp LANGUAGES CXX VERSION 2.0.0 DESCRIPTION "My CMake Project")
add_executable(myapp main.cpp)
"#
  );
  Ok(())
}
