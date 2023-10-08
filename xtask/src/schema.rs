use anyhow::{Context, Result};
use ast_grep_config::SerializableRuleConfig;
use ast_grep_core::{language::TSLanguage, Language};
use schemars::{
  gen::SchemaGenerator,
  schema::{InstanceType, Schema, SchemaObject},
  schema_for, JsonSchema,
};
use serde_json::to_writer_pretty;

use std::borrow::Cow;
use std::fs::File;

pub fn generate_schema() -> Result<()> {
  let schema = schema_for!(SerializableRuleConfig<PlaceholderLang>);
  let mut file = File::create("schemas/rule.json")?;
  to_writer_pretty(&mut file, &schema).context("cannot print JSON schema")
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
