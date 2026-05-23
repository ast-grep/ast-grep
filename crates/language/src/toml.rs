#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_non_match_lang, test_replace_lang};

fn test_match(query: &str, source: &str) {
  test_match_lang(query, source, Toml);
}

fn test_non_match(query: &str, source: &str) {
  test_non_match_lang(query, source, Toml);
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  test_replace_lang(src, pattern, replacer, Toml)
}

// --- Basic value matching ---

#[test]
fn test_toml_string_pair() {
  test_match(
    r#"name = "foo""#,
    "[package]\nname = \"foo\"\nversion = \"1.0\"",
  );
  // non-match by key name, not by value (string content is anonymous in the grammar)
  test_non_match(
    r#"description = "foo""#,
    "[package]\nname = \"foo\"\nversion = \"1.0\"",
  );
}

#[test]
fn test_toml_integer_pair() {
  test_match("port = 8080", "[server]\nport = 8080\nhost = \"localhost\"");
  test_non_match("port = 8080", "[server]\nport = 3000\nhost = \"localhost\"");
}

#[test]
fn test_toml_boolean_pair() {
  test_match("flag = true", "[options]\nflag = true\nverbose = false");
  test_non_match("flag = true", "[options]\nflag = false\nverbose = false");
}

// --- Pattern matching with meta variables ---

#[test]
fn test_toml_meta_var_value() {
  test_match(
    "name = $VAL",
    "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"",
  );
  test_match("port = $VAL", "[server]\nport = 8080\nhost = \"localhost\"");
  test_match("flag = $VAL", "[options]\nflag = true\nverbose = false");
}

#[test]
fn test_toml_meta_var_non_match() {
  test_non_match(
    "missing_key = $VAL",
    "[package]\nname = \"foo\"\nversion = \"1.0\"",
  );
}

// --- Cargo.toml: package metadata ---

const CARGO_TOML: &str = r#"[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
authors = ["Alice <alice@example.com>"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = "1.0"
log = "0.4"

[dev-dependencies]
pretty_assertions = "1.4"
"#;

#[test]
fn test_cargo_find_package_name() {
  test_match(r#"name = "my-crate""#, CARGO_TOML);
}

#[test]
fn test_cargo_find_version() {
  test_match(r#"version = "0.1.0""#, CARGO_TOML);
}

#[test]
fn test_cargo_find_edition() {
  test_match(r#"edition = "2021""#, CARGO_TOML);
}

#[test]
fn test_cargo_find_simple_dep() {
  test_match(r#"tokio = "1.0""#, CARGO_TOML);
}

#[test]
fn test_cargo_find_dep_with_features() {
  test_match(
    r#"serde = { version = "1.0", features = ["derive"] }"#,
    CARGO_TOML,
  );
}

#[test]
fn test_cargo_non_match_missing_dep() {
  test_non_match(r#"rand = "0.8""#, CARGO_TOML);
}

// --- Inline tables ---

#[test]
fn test_toml_inline_table() {
  test_match(
    r#"version = "1.0""#,
    "[deps]\nserde = { version = \"1.0\", features = [\"derive\"] }",
  );
}

// --- Arrays ---

#[test]
fn test_toml_array() {
  test_match(
    r#"features = ["derive"]"#,
    "[deps]\nfoo = { version = \"1.0\" }\nfeatures = [\"derive\"]",
  );
}

// --- Replace ---

#[test]
fn test_toml_replace_version() {
  let ret = test_replace(
    "[package]\nversion = \"0.1.0\"\nedition = \"2021\"",
    r#"version = "0.1.0""#,
    r#"version = "0.2.0""#,
  );
  assert_eq!(ret, "[package]\nversion = \"0.2.0\"\nedition = \"2021\"");
}

#[test]
fn test_toml_replace_edition() {
  let ret = test_replace(
    "[package]\nname = \"foo\"\nedition = \"2021\"",
    r#"edition = "2021""#,
    r#"edition = "2024""#,
  );
  assert_eq!(ret, "[package]\nname = \"foo\"\nedition = \"2024\"");
}

// --- Table headers ---

#[test]
fn test_toml_table_header() {
  test_match("[package]", "[package]\nname = \"foo\"");
  test_match("[dependencies]", "[dependencies]\nfoo = \"1.0\"");
}

// --- Dotted keys ---

#[test]
fn test_toml_dotted_key() {
  test_match(
    "authors.workspace = true",
    "[package]\nname = \"foo\"\nauthors.workspace = true",
  );
}

// --- Nested Cargo.toml workspace patterns ---

const WORKSPACE_TOML: &str = r#"[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.42.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
"#;

#[test]
fn test_workspace_resolver() {
  test_match(r#"resolver = "2""#, WORKSPACE_TOML);
}

#[test]
fn test_workspace_version() {
  test_match(r#"version = "0.42.0""#, WORKSPACE_TOML);
}

#[test]
fn test_workspace_license() {
  test_match(r#"license = "MIT""#, WORKSPACE_TOML);
}

// --- Advanced: meta variable replacement in Cargo.toml ---

#[test]
fn test_cargo_replace_dep_version() {
  let src = "[dependencies]\ntokio = \"1.0\"\nlog = \"0.4\"";
  let ret = test_replace(src, r#"tokio = "1.0""#, r#"tokio = "2.0""#);
  assert_eq!(ret, "[dependencies]\ntokio = \"2.0\"\nlog = \"0.4\"");
}

// --- Array of tables ---

const ARRAY_TABLE_TOML: &str = r#"[[bin]]
name = "my-app"
path = "src/main.rs"

[[bin]]
name = "other-app"
path = "src/other.rs"
"#;

#[test]
fn test_toml_array_of_tables() {
  test_match(r#"name = "my-app""#, ARRAY_TABLE_TOML);
  test_match(r#"path = "src/main.rs""#, ARRAY_TABLE_TOML);
  test_non_match(r#"missing = "value""#, ARRAY_TABLE_TOML);
}

#[test]
fn test_toml_array_table_path() {
  test_match(r#"path = "src/main.rs""#, ARRAY_TABLE_TOML);
}

// =============================================================================
// Creative edge-case tests — looking for matcher bugs around TOML literals,
// quoting, escapes, special floats, dates, keys, containers, and replacements.
// =============================================================================

// --- Empty strings: pattern `""` matches only `""` ---
// The generic "content absorbed into parent" detector doesn't fire on empty
// literals (no uncovered bytes), so TOML's `(string)` is also declared atomic
// via the `kind_is_atomic` hook in crates/language/src/lib.rs.

#[test]
fn test_empty_string_matches_empty() {
  test_match(r#"x = """#, r#"x = """#);
}

#[test]
fn test_nonempty_does_not_match_empty() {
  test_non_match(r#"x = "foo""#, r#"x = """#);
}

#[test]
fn test_empty_string_vs_nonempty() {
  test_non_match(r#"x = """#, r#"x = "foo""#);
}

#[test]
fn test_empty_literal_string_vs_nonempty() {
  test_non_match("x = ''", "x = 'foo'");
}

#[test]
fn test_empty_multiline_string_vs_nonempty() {
  test_non_match("x = \"\"\"\"\"\"", "x = \"\"\"foo\"\"\"");
}

// --- Mixed quote types: basic ("foo") vs literal ('foo') ---

#[test]
fn test_basic_string_does_not_match_literal_string() {
  // Different node kinds (basic vs literal string) — must not cross-match.
  test_non_match(r#"x = "foo""#, "x = 'foo'");
}

#[test]
fn test_literal_string_does_not_match_basic_string() {
  test_non_match("x = 'foo'", r#"x = "foo""#);
}

// --- Escape sequences (escape_sequence is a NAMED child) ---

#[test]
fn test_string_with_escape_value_distinct() {
  test_non_match(r#"x = "a\nb""#, r#"x = "a\tb""#);
}

#[test]
fn test_string_unicode_escape_value_distinct() {
  test_non_match(r#"x = "A""#, r#"x = "B""#);
}

// Note: there is no test for `"$VAR"` in a TOML string vs a literal `$VAR`
// in a TOML source. Toml's `pre_process_pattern` replaces `$VAR` with `_VAR`
// in the pattern (so `$VAR` can be a metavariable), which means the literal
// string `$VAR` in source TOML is not directly addressable as a pattern.
// This is an inherent expando-char tradeoff shared by every ast-grep language
// with that mechanism, not a bug specific to TOML.

// --- Special float values ---

#[test]
fn test_float_inf_vs_neg_inf() {
  test_non_match("x = inf", "x = -inf");
}

#[test]
fn test_float_inf_vs_nan() {
  test_non_match("x = inf", "x = nan");
}

#[test]
fn test_float_inf_matches_inf() {
  test_match("x = inf", "x = inf");
}

#[test]
fn test_float_underscore_separator() {
  test_match("x = 1_000_000", "x = 1_000_000");
  // Source uses underscores; pattern without underscores has different text
  // (the underscore IS in the integer token, so token text differs).
  test_non_match("x = 1000000", "x = 1_000_000");
}

// --- Integer bases ---

#[test]
fn test_hex_vs_decimal_same_value() {
  // 0xFF and 255 have the same numeric value but different token text.
  test_non_match("x = 0xFF", "x = 255");
}

#[test]
fn test_octal_value_distinct() {
  test_non_match("x = 0o755", "x = 0o644");
}

#[test]
fn test_binary_value_distinct() {
  test_non_match("x = 0b1010", "x = 0b0101");
}

// --- Date / time edge cases ---

#[test]
fn test_offset_date_time_offset_distinct() {
  test_non_match(
    "x = 2020-01-01T00:00:00+00:00",
    "x = 2020-01-01T00:00:00+05:00",
  );
}

#[test]
fn test_local_time_distinct() {
  test_non_match("x = 12:00:00", "x = 13:00:00");
}

#[test]
fn test_date_vs_datetime() {
  // Same date but different AST kind (local_date vs local_date_time).
  test_non_match("x = 2020-01-01", "x = 2020-01-01T00:00:00");
}

// --- Empty containers ---

#[test]
fn test_empty_array_does_not_match_nonempty() {
  test_non_match("x = []", "x = [1]");
}

#[test]
fn test_nonempty_array_does_not_match_empty() {
  test_non_match("x = [1]", "x = []");
}

#[test]
fn test_empty_inline_table_does_not_match_nonempty() {
  test_non_match("x = {}", "x = { a = 1 }");
}

#[test]
fn test_array_size_distinct() {
  test_non_match("x = [1, 2]", "x = [1, 2, 3]");
  test_non_match("x = [1, 2, 3]", "x = [1, 2]");
}

#[test]
fn test_array_order_distinct() {
  test_non_match("x = [1, 2, 3]", "x = [3, 2, 1]");
}

// --- Keys ---

#[test]
fn test_quoted_key_vs_bare_key() {
  // Same effective key name but different AST kind (quoted_key vs bare_key).
  test_non_match("\"foo\" = 1", "foo = 1");
}

#[test]
fn test_literal_quoted_key_vs_basic_quoted_key() {
  test_non_match("'foo' = 1", "\"foo\" = 1");
}

#[test]
fn test_dotted_key_value_distinct() {
  // Same dotted shape but different leaf key.
  test_non_match("a.b.c = 1", "a.b.d = 1");
}

#[test]
fn test_numeric_bare_key() {
  test_match("2020 = \"year\"", "2020 = \"year\"");
  test_non_match("2020 = \"year\"", "2021 = \"year\"");
}

#[test]
fn test_key_with_hyphen() {
  test_match("foo-bar = 1", "foo-bar = 1");
  test_non_match("foo-bar = 1", "foo_bar = 1");
}

// --- Inline tables: structural matching ---

#[test]
fn test_inline_table_extra_field_distinct() {
  test_non_match("a = { b = 1 }", "a = { b = 1, c = 2 }");
}

#[test]
fn test_inline_table_missing_field_distinct() {
  test_non_match("a = { b = 1, c = 2 }", "a = { b = 1 }");
}

#[test]
fn test_inline_table_value_distinct() {
  test_non_match("a = { b = 1 }", "a = { b = 2 }");
}

// --- Multiline strings ---

#[test]
fn test_multiline_basic_string_does_not_match_singleline() {
  // Different node shape (multiline_basic vs basic string).
  test_non_match("x = \"\"\"foo\"\"\"", "x = \"foo\"");
}

#[test]
fn test_multiline_literal_string_value_distinct() {
  test_non_match("x = '''foo'''", "x = '''bar'''");
}

#[test]
fn test_multiline_string_with_newline_value_distinct() {
  test_non_match("x = \"\"\"\nfoo\n\"\"\"", "x = \"\"\"\nbar\n\"\"\"");
}

// --- Comments don't affect matching ---

#[test]
fn test_pattern_ignores_source_comments() {
  test_match("port = 8080", "# server port\nport = 8080");
}

// --- Replace operations ---

#[test]
fn test_replace_value_in_dotted_key() {
  let ret = test_replace("a.b = \"old\"", r#"a.b = "old""#, r#"a.b = "new""#);
  assert_eq!(ret, "a.b = \"new\"");
}

#[test]
fn test_replace_only_matching_dep_among_many() {
  // A regression target for the bug we fixed: ensure replace only touches the
  // specific dep whose value matches.
  let src = "[deps]\nserde = \"1.0\"\ntokio = \"1.0\"\nlog = \"0.4\"";
  let ret = test_replace(src, r#"tokio = "1.0""#, r#"tokio = "2.0""#);
  assert_eq!(
    ret,
    "[deps]\nserde = \"1.0\"\ntokio = \"2.0\"\nlog = \"0.4\""
  );
}

#[test]
fn test_replace_with_metavar_capture() {
  // Capture the value, then substitute. The captured string node must
  // round-trip its content correctly.
  let ret = test_replace("name = \"hello\"", r#"name = $V"#, r#"label = $V"#);
  assert_eq!(ret, "label = \"hello\"");
}

#[test]
fn test_replace_does_not_swap_string_kinds() {
  // Pattern is basic string; source has literal string — should NOT match,
  // so no replacement.
  let mut source = Toml.ast_grep("x = 'foo'");
  let replaced = source.replace(r#"x = "foo""#, r#"x = "bar""#).expect("ok");
  assert!(!replaced);
  assert_eq!(source.generate(), "x = 'foo'");
}

// --- Boolean ---

#[test]
fn test_boolean_case_sensitivity() {
  // TOML booleans are lowercase. `True` is not a TOML boolean — it'd be
  // parsed as a bare key or error. Pattern must not match.
  test_non_match("x = True", "x = true");
}

// --- Whitespace insensitivity ---

#[test]
fn test_extra_whitespace_around_equals() {
  test_match("x = 1", "x   =   1");
}

#[test]
fn test_extra_blank_lines() {
  test_match("[package]\nname = \"foo\"", "[package]\n\n\nname = \"foo\"");
}

// --- Value-distinction tests ---
// Integer / boolean / float / date pass; string variants FAIL today because
// tree-sitter-toml-ng exposes string contents only as anonymous tokens, so
// the matcher sees every `(string)` node as equivalent regardless of text.

#[test]
fn test_integer_value_distinct() {
  test_non_match("port = 8080", "port = 3000");
}

#[test]
fn test_boolean_value_distinct() {
  test_non_match("flag = true", "flag = false");
}

#[test]
fn test_float_value_distinct() {
  test_non_match("x = 3.14", "x = 2.71");
}

#[test]
fn test_date_value_distinct() {
  test_non_match("x = 2020-01-01", "x = 1999-12-31");
}

#[test]
fn test_basic_string_value_distinct() {
  // FAILS today: pattern `name = "bar"` wrongly matches source `name = "foo"`.
  test_non_match(r#"name = "bar""#, r#"name = "foo""#);
}

#[test]
fn test_literal_string_value_distinct() {
  // FAILS today: single-quoted literal strings also indistinguishable.
  test_non_match("x = 'bar'", "x = 'foo'");
}

#[test]
fn test_multiline_string_value_distinct() {
  // FAILS today: triple-quoted multi-line strings also indistinguishable.
  test_non_match("x = \"\"\"world\n\"\"\"", "x = \"\"\"hello\n\"\"\"");
}

#[test]
fn test_toml_meta_var_key() {
  // Verifies the expando_char='_' choice: $KEY is rewritten to `_KEY`, which
  // is a valid TOML bare key (bare_key = /[A-Za-z0-9_-]+/).
  test_match("$KEY = 8080", "port = 8080");
  test_match("$KEY = \"foo\"", "name = \"foo\"");
}

#[test]
fn test_toml_meta_var_both_sides() {
  test_match("$KEY = $VAL", "port = 8080");
  test_match("$KEY = $VAL", "name = \"foo\"");
  test_match("$KEY = $VAL", "flag = true");
}

#[test]
fn test_toml_negative_integer() {
  test_match("x = -3", "x = -3");
  test_non_match("x = -3", "x = 3");
}

#[test]
fn test_toml_hex_integer() {
  test_match("x = 0xFF", "x = 0xFF");
  test_non_match("x = 0xFF", "x = 0xAB");
}

#[test]
fn test_toml_comment() {
  test_match("# todo", "# todo\nx = 1");
}

#[test]
fn test_replace_should_respect_string_value() {
  // Replacing `version = "0.1.0"` must leave `version = "9.9.9"` untouched —
  // string content must be compared. Before the fix in
  // crates/core/src/matcher/pattern.rs this replaced 9.9.9 with 2.0.0.
  let src = "[package]\nversion = \"9.9.9\"\nedition = \"2021\"";
  let mut source = Toml.ast_grep(src);
  let replaced = source
    .replace(r#"version = "0.1.0""#, r#"version = "2.0.0""#)
    .expect("should parse");
  assert!(!replaced, "should not match a different string value");
  assert_eq!(source.generate(), src);
}
