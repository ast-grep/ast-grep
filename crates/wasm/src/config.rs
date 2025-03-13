use ast_grep_config::{
  DeserializeEnv, RuleConfig, RuleCore, RuleCoreError, SerializableRuleConfig, SerializableRuleCore,
};
use ast_grep_language::SupportLang;
use thiserror::Error;
use wasm_bindgen::{JsError, JsValue};

#[derive(Error, Debug)]
pub enum WasmConfigError {
  #[error("Fail to parse yaml as RuleConfig")]
  Parse(#[from] serde_wasm_bindgen::Error),

  #[error("Fail to parse yaml as Rule.")]
  Core(#[from] RuleCoreError),
}

pub fn try_get_rule_config(config: JsValue) -> Result<RuleConfig<SupportLang>, JsError> {
  let config: SerializableRuleConfig<SupportLang> = serde_wasm_bindgen::from_value(config)?;
  RuleConfig::try_from(config, &Default::default()).map_err(dump_error)
}

pub fn parse_config_from_js_value(
  lang: SupportLang,
  rule: JsValue,
) -> Result<RuleCore<SupportLang>, WasmConfigError> {
  let mut rule: SerializableRuleCore =
    serde_wasm_bindgen::from_value(rule).map_err(WasmConfigError::Parse)?;
  rule.fix = None;
  let env = DeserializeEnv::new(lang);
  rule.get_matcher(env).map_err(WasmConfigError::Core)
}

fn dump_error(err: impl std::error::Error) -> JsError {
  let mut errors = vec![err.to_string()];
  let mut err: &dyn std::error::Error = &err;
  while let Some(e) = err.source() {
    errors.push(e.to_string());
    err = e;
  }
  JsError::new(&errors.join("\n").to_string())
}
