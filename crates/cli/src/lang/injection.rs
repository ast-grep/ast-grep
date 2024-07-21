use super::SgLang;
use ast_grep_config::SerializableRuleCore;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Injected {
  Static(SgLang),
  Dynamic(Vec<SgLang>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanguageInjection {
  #[serde(flatten)]
  core: SerializableRuleCore,
  /// The host language, e.g. html, contains other languages
  host_language: SgLang,
  /// Injected language according to the rule
  /// It accepts either a string like js for single static language.
  /// or an array of string like [js, ts] for dynamic language detection.
  injected: Injected,
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::from_str;
  const DYNAMIC: &str = "
hostLanguage: HTML
rule:
  pattern: <script lang=$LANG>$CONTENT</script>
injected: [js, ts, tsx]";
  const STATIC: &str = "
hostLanguage: HTML
rule:
  pattern: <script>$CONTENT</script>
injected: js";
  #[test]
  fn test_deserialize() {
    let inj: LanguageInjection = from_str(STATIC).expect("should ok");
    assert!(matches!(inj.injected, Injected::Static(_)));
    let inj: LanguageInjection = from_str(DYNAMIC).expect("should ok");
    assert!(matches!(inj.injected, Injected::Dynamic(_)));
  }
}
