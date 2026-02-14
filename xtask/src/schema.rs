use anyhow::{Context, Result};
use ast_grep_config::SerializableRuleConfig;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, TSLanguage};
use ast_grep_core::Language;
use ast_grep_language::{
  Alias, Bash, CSharp, Cpp, Css, Elixir, Go, Haskell, Html, Java, JavaScript, Json, Kotlin, Lua,
  Php, Python, Ruby, Rust, Scala, Swift, SystemVerilog, Tsx, TypeScript, Yaml, C,
};
use schemars::{json_schema, schema_for, JsonSchema, Schema, SchemaGenerator};
use serde_json::{to_writer_pretty, Value};

use std::borrow::Cow;
use std::{collections::BTreeSet, fs::File};

pub fn generate_schema() -> Result<()> {
  let schema = schema_for!(SerializableRuleConfig<PlaceholderLang>);
  generate_lang_schemas()?;
  // use manifest to locate schema. "schemas/rule.json" only works when cwd is root dir
  // however, pwd is set to manifest dir, xtask in this case, during cargo test
  let xtask_path = std::env::var("CARGO_MANIFEST_DIR")?;
  let rule_path = std::fs::canonicalize(format!("{xtask_path}/../schemas/rule.json"))?;
  let mut file = File::create(rule_path)?;
  to_writer_pretty(&mut file, &schema).context("cannot print JSON schema")
}

fn generate_lang_schemas() -> Result<()> {
  generate_lang_schema(Bash, "bash")?;
  generate_lang_schema(C, "c")?;
  generate_lang_schema(Cpp, "cpp")?;
  generate_lang_schema(CSharp, "csharp")?;
  generate_lang_schema(Css, "css")?;
  generate_lang_schema(Go, "go")?;
  generate_lang_schema(Elixir, "elixir")?;
  generate_lang_schema(Haskell, "haskell")?;
  generate_lang_schema(Html, "html")?;
  generate_lang_schema(Java, "java")?;
  generate_lang_schema(JavaScript, "javascript")?;
  generate_lang_schema(Json, "json")?;
  generate_lang_schema(Kotlin, "kotlin")?;
  generate_lang_schema(Lua, "lua")?;
  generate_lang_schema(Php, "php")?;
  generate_lang_schema(Python, "python")?;
  generate_lang_schema(Ruby, "ruby")?;
  generate_lang_schema(Rust, "rust")?;
  generate_lang_schema(Scala, "scala")?;
  generate_lang_schema(SystemVerilog, "systemverilog")?;
  generate_lang_schema(Swift, "swift")?;
  generate_lang_schema(Tsx, "tsx")?;
  generate_lang_schema(TypeScript, "typescript")?;
  generate_lang_schema(Yaml, "yaml")
}

fn generate_lang_schema<T: LanguageExt + Alias>(lang: T, name: &str) -> Result<()> {
  let mut schema = schema_for!(SerializableRuleConfig<PlaceholderLang>);
  add_lang_info_to_schema(&mut schema, lang, name)?;
  let xtask_path = std::env::var("CARGO_MANIFEST_DIR")?;
  let rule_path = std::fs::canonicalize(format!("{xtask_path}/../schemas/{name}_rule.json"))?;
  let mut file = File::create(rule_path)?;
  to_writer_pretty(&mut file, &schema).context("cannot print JSON schema")
}

fn add_lang_info_to_schema<T: LanguageExt + Alias>(
  schema: &mut Schema,
  lang: T,
  name: &str,
) -> Result<()> {
  // change rule title
  let title = schema.get_mut("title").context("must have title")?;
  *title = Value::String(format!("ast-grep rule for {name}"));

  let definitions = schema.get_mut("$defs").context("must have definitions")?;

  // insert field to relation
  let relation = definitions
    .get_mut("Relation")
    .context("must have relation")?;
  let relation_props = relation
    .get_mut("properties")
    .context("must have properties")?;
  let field = relation_props
    .get_mut("field")
    .context("must have field")?
    .as_object_mut()
    .context("field must be an object")?;
  field.insert(
    "enum".to_string(),
    Value::Array(get_fields(&lang.get_ts_language())),
  );

  // insert kind to relation and rule
  insert_kind(relation_props, &lang)?;
  let rule = definitions
    .get_mut("SerializableRule")
    .context("must have SerializableRule")?;
  let rule_props = rule.get_mut("properties").context("must have properties")?;
  insert_kind(rule_props, &lang)?;

  // insert language
  let language = definitions
    .get_mut("Language")
    .context("must have Language")?
    .as_object_mut()
    .context("Language must be an object")?;
  language.insert(
    "enum".to_string(),
    Value::Array(
      T::ALIAS
        .iter()
        .map(|alias| Value::String(alias.to_string()))
        .chain(std::iter::once(Value::String(lang.to_string())))
        .collect(),
    ),
  );
  Ok(())
}

fn insert_kind<L: LanguageExt>(schema: &mut Value, lang: &L) -> Result<()> {
  let named_nodes = get_named_nodes(&lang.get_ts_language());
  let kind = schema
    .get_mut("kind")
    .context("must have kind")?
    .as_object_mut()
    .context("kind must be an object")?;
  kind.insert("enum".to_string(), Value::Array(named_nodes));
  Ok(())
}

fn get_named_nodes(lang: &TSLanguage) -> Vec<Value> {
  let enum_values = BTreeSet::from_iter((0..lang.node_kind_count()).filter_map(|id| {
    if lang.node_kind_is_named(id as u16) {
      lang
        .node_kind_for_id(id as u16)
        .map(|kind| kind.to_string())
    } else {
      None
    }
  }));

  enum_values
    .into_iter()
    .map(serde_json::Value::String)
    .collect()
}

fn get_fields(lang: &TSLanguage) -> Vec<Value> {
  let enum_values = BTreeSet::from_iter(
    // Field IDs start from 1 in tree-sitter.
    (1..lang.field_count())
      .filter_map(|id| lang.field_name_for_id(id as u16).map(|s| s.to_string())),
  );

  enum_values
    .into_iter()
    .map(serde_json::Value::String)
    .collect()
}

#[derive(Clone)]
struct PlaceholderLang;
// reference: https://github.com/GREsau/schemars/blob/9415fcb57b85f12e07afeb1dd16184bab0e26a84/schemars/src/json_schema_impls/primitives.rs#L8
impl JsonSchema for PlaceholderLang {
  fn schema_id() -> Cow<'static, str> {
    Cow::Borrowed("Language")
  }
  fn schema_name() -> Cow<'static, str> {
    Cow::Borrowed("Language")
  }
  fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
    json_schema!({
      "type": "string",
      "description": "Placeholder for language, used in JSON schema only.",
      "example": "typescript"
    })
  }
}

impl Language for PlaceholderLang {
  fn kind_to_id(&self, _kind: &str) -> u16 {
    unreachable!("PlaceholderLang is only for json schema")
  }
  fn field_to_id(&self, _field: &str) -> Option<u16> {
    unreachable!("PlaceholderLang is only for json schema")
  }
  fn build_pattern(&self, _b: &PatternBuilder) -> Result<Pattern, PatternError> {
    unreachable!("PlaceholderLang is only for json schema")
  }
}
impl LanguageExt for PlaceholderLang {
  fn get_ts_language(&self) -> TSLanguage {
    unreachable!("PlaceholderLang is only for json schema")
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_json_schema() {
    let ret = generate_schema();
    assert!(ret.is_ok());
  }
}
