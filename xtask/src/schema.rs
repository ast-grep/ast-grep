use anyhow::{bail, Context, Result};
use ast_grep_config::SerializableRuleConfig;
use ast_grep_core::{language::TSLanguage, Language};
use schemars::{
  gen::SchemaGenerator,
  schema::{InstanceType, RootSchema, Schema, SchemaObject},
  schema_for, JsonSchema,
};
use serde_json::to_writer_pretty;

use std::borrow::Cow;
use std::fs::File;

pub fn generate_schema() -> Result<()> {
  let mut schema = schema_for!(SerializableRuleConfig<PlaceholderLang>);
  tweak_schema(&mut schema)?;
  let mut file = File::create("schemas/rule.json")?;
  to_writer_pretty(&mut file, &schema).context("cannot print JSON schema")
}

fn tweak_schema(schema: &mut RootSchema) -> Result<()> {
  // better schema name
  schema.schema.metadata().title = Some("ast-grep rule".to_string());
  // using rule/relation will be too noisy
  let description = remove_recursive_rule_relation_description(schema)?;
  // set description to rule
  let props = &mut schema.schema.object().properties;
  let Schema::Object(rule) = props.get_mut("rule").context("must have rule")? else {
    bail!("rule's type is not object!");
  };
  rule.metadata().description = description;
  Ok(())
}

fn remove_recursive_rule_relation_description(schema: &mut RootSchema) -> Result<Option<String>> {
  let definitions = &mut schema.definitions;
  let Schema::Object(relation) = definitions.get_mut("Relation").context("must have relation")? else {
    bail!("Relation's type is not object!");
  };
  relation.metadata().description = None;
  let Schema::Object(rule) = definitions.get_mut("SerializableRule").context("must have rule")? else {
    bail!("SerializableRule's type is not object!");
  };
  Ok(rule.metadata().description.take())
}

#[derive(Clone)]
struct PlaceholderLang;
// reference: https://github.com/GREsau/schemars/blob/9415fcb57b85f12e07afeb1dd16184bab0e26a84/schemars/src/json_schema_impls/primitives.rs#L8
impl JsonSchema for PlaceholderLang {
  fn schema_id() -> std::borrow::Cow<'static, str> {
    Cow::Borrowed("Language")
  }
  fn schema_name() -> String {
    String::from("Language")
  }
  fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
    SchemaObject {
      instance_type: Some(InstanceType::String.into()),
      format: None,
      ..Default::default()
    }
    .into()
  }
}

impl Language for PlaceholderLang {
  fn get_ts_language(&self) -> TSLanguage {
    unreachable!("PlaceholderLang is only for json schema")
  }
}
