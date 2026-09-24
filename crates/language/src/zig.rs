#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_replace_lang};

fn test_match(s1: &str, s2: &str) {
  test_match_lang(s1, s2, Zig)
}

#[test]
fn test_zig_pattern() {
  // tree-sitter-zig 1.1.2 + expando '_', verified on Zig 0.16.0.
  test_match("const $A = @import($B);", "const std = @import(\"std\");");
  test_match("$A + $B", "a + b");
  test_match("return $A;", "return a + b;");
}

#[test]
fn test_zig_function() {
  test_match(
    "fn $NAME($$$ARGS) !void { $$$BODY }",
    "fn main() !void { return; }",
  );
  test_match(
    "pub fn $NAME($$$ARGS) $RET { $$$BODY }",
    "pub fn add(a: i32, b: i32) i32 { return a + b; }",
  );
}

#[test]
fn test_zig_call() {
  test_match("$F($$$ARGS)", "add(1, 2)");
  test_match("$F($A, $B)", "add(1, 2)");
}

#[test]
fn test_zig_016_idioms() {
  test_match(
    "var $A: $T = .empty;",
    "var list: std.ArrayList(u8) = .empty;",
  );
  test_match(
    "const $A: $T = .{ $$$FIELDS };",
    "const p: Point = .{ .x = 1, .y = 2 };",
  );
  test_match(
    "var $A: $T = .{};",
    "var gpa: std.heap.GeneralPurposeAllocator(.{}) = .{};",
  );
  test_match(
    "pub const $NAME = struct { $$$FIELDS };",
    "pub const Point = struct { x: i32, y: i32, };",
  );
}

#[test]
fn test_zig_control_flow() {
  test_match(
    "switch ($A) { $$$ARMS }",
    "switch (sum) { 0 => {}, else => {}, }",
  );
  test_match("$A catch $B", "eu catch 0");
  test_match("try $A;", "try maybeFail(true);");
  test_match("break :$L $V;", "break :blk 7;");
  // `defer` is deliberately absent: `defer $A;` parses as multiple top-level
  // nodes, so ast-grep rejects the pattern with "Multiple AST nodes are
  // detected". Match it by kind instead (rule `kind: defer_statement`).
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  test_replace_lang(src, pattern, replacer, Zig)
}

#[test]
fn test_zig_replace() {
  let ret = test_replace("a + b", "$A + $B", "$B + $A");
  assert_eq!(ret, "b + a");

  let ret = test_replace(
    "const std = @import(\"std\");",
    "const $A = @import($B);",
    "const $A = @import(\"builtin\");",
  );
  assert_eq!(ret, "const std = @import(\"builtin\");");
}
