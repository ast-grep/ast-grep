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
  test_match("name = $VAL", "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"");
  test_match("port = $VAL", "[server]\nport = 8080\nhost = \"localhost\"");
  test_match("flag = $VAL", "[options]\nflag = true\nverbose = false");
}

#[test]
fn test_toml_meta_var_non_match() {
  test_non_match("missing_key = $VAL", "[package]\nname = \"foo\"\nversion = \"1.0\"");
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