use anyhow::{bail, Context, Result};
use ast_grep_config::SerializableRuleConfig;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, TSLanguage};
use ast_grep_core::Language;
use ast_grep_language::{
  Alias, Bash, CSharp, Cpp, Css, Elixir, Go, Haskell, Html, Java, JavaScript, Json, Kotlin, Lua,
  Php, Python, Ruby, Rust, Scala, Swift, Tsx, TypeScript, Yaml, C,
};
use schemars::{json_schema, schema_for, JsonSchema, Schema, SchemaGenerator};
use serde_json::{to_writer_pretty, Value};

use std::borrow::Cow;
use std::{collections::BTreeSet, fs::File};

pub fn generate_schema() -> Result<()> {
  let mut schema = schema_for!(SerializableRuleConfig<PlaceholderLang>);
  tweak_schema(&mut schema)?;
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
  generate_lang_schema(Swift, "swift")?;
  generate_lang_schema(Tsx, "tsx")?;
  generate_lang_schema(TypeScript, "typescript")?;
  generate_lang_schema(Yaml, "yaml")
}

fn generate_lang_schema<T: LanguageExt + Alias>(lang: T, name: &str) -> Result<()> {
  let mut schema = schema_for!(SerializableRuleConfig<PlaceholderLang>);
  tweak_schema(&mut schema)?;
  add_lang_info_to_schema(&mut schema, lang, name)?;
  let xtask_path = std::env::var("CARGO_MANIFEST_DIR")?;
  let rule_path = std::fs::canonicalize(format!("{xtask_path}/../schemas/{name}_rule.json"))?;
  let mut file = File::create(rule_path)?;
  to_writer_pretty(&mut file, &schema).context("cannot print JSON schema")
}

fn tweak_schema(schema: &mut Schema) -> Result<()> {
  // better schema name
  // schema.schema.metadata().title = Some("ast-grep rule".to_string());
  // // stopby's rule does not need to be nested
  // simplify_stop_by(schema)?;
  // // using rule/relation will be too noisy
  // let description = remove_recursive_rule_relation_description(schema)?;
  // // set description to rule
  // let props = &mut schema.schema.object().properties;
  // let Schema::Object(rule) = props.get_mut("rule").context("must have rule")? else {
  //   bail!("rule's type is not object!");
  // };
  // rule.metadata().description = description;
  Ok(())
}

fn add_lang_info_to_schema<T: LanguageExt + Alias>(
  schema: &mut Schema,
  lang: T,
  name: &str,
) -> Result<()> {
  // schema.schema.metadata().title = Some(format!("ast-grep rule for {name}"));
  // let definitions = &mut schema.definitions;
  // let Schema::Object(relation) = definitions
  //   .get_mut("Relation")
  //   .context("must have relation")?
  // else {
  //   bail!("Relation's type is not object!");
  // };
  // let relation_props = &mut relation.object().properties;
  // let Schema::Object(field) = relation_props.get_mut("field").context("must have field")? else {
  //   bail!("field's type is not object!")
  // };
  // field.enum_values = Some(get_fields(&lang.get_ts_language()));
  // let Schema::Object(kind) = relation_props.get_mut("kind").context("must have kind")? else {
  //   bail!("kind's type is not object!")
  // };
  // let named_nodes = get_named_nodes(&lang.get_ts_language());
  // kind.enum_values = Some(named_nodes.clone());
  // let Schema::Object(serializable_rule) = definitions
  //   .get_mut("SerializableRule")
  //   .context("must have SerializableRule")?
  // else {
  //   bail!("SerializableRule's type is not object!");
  // };
  // let serializable_rule_props = &mut serializable_rule.object().properties;
  // let Schema::Object(kind) = serializable_rule_props
  //   .get_mut("kind")
  //   .context("must have kind")?
  // else {
  //   bail!("kind's type is not object!")
  // };
  // kind.enum_values = Some(named_nodes);
  // let Schema::Object(language) = definitions
  //   .get_mut("Language")
  //   .context("must have Language")?
  // else {
  //   bail!("Language's type is not an object!")
  // };
  // language.enum_values = Some(
  //   T::ALIAS
  //     .iter()
  //     .map(|alias| serde_json::Value::String(alias.to_string()))
  //     .chain(std::iter::once(serde_json::Value::String(format!(
  //       "{lang}"
  //     ))))
  //     .collect(),
  // );
  Ok(())
}

fn remove_recursive_rule_relation_description(schema: &mut Schema) -> Result<Option<String>> {
  // let definitions = &mut schema.definitions;
  // let Schema::Object(relation) = definitions
  //   .get_mut("Relation")
  //   .context("must have relation")?
  // else {
  //   bail!("Relation's type is not object!");
  // };
  // relation.metadata().description = None;
  // let Schema::Object(rule) = definitions
  //   .get_mut("SerializableRule")
  //   .context("must have rule")?
  // else {
  //   bail!("SerializableRule's type is not object!");
  // };
  // Ok(rule.metadata().description.take())
  Ok(None)
}

fn simplify_stop_by(schema: &mut Schema) -> Result<()> {
  // let definitions = &mut schema.definitions;
  // let Schema::Object(stop_by) = definitions
  //   .get_mut("SerializableStopBy")
  //   .context("must have stopby")?
  // else {
  //   bail!("StopBy's type is not object!");
  // };
  // let one_ofs = stop_by
  //   .subschemas()
  //   .one_of
  //   .as_mut()
  //   .context("should have one_of")?;
  // let Schema::Object(rule) = &mut one_ofs[1] else {
  //   bail!("type is not object!");
  // };
  // let rule = rule
  //   .object()
  //   .properties
  //   .remove("rule")
  //   .context("should have rule")?;
  // one_ofs[1] = rule;
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
