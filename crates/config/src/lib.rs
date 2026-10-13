mod check_var;
mod combined;
mod fixer;
mod label;
mod maybe;
mod rewriter;
mod rule;
mod rule_collection;
mod rule_config;
mod rule_core;
mod transform;

use serde::Deserialize;
use serde_saphyr::{Error as YamlError, MergeKeyPolicy, NonFiniteFloatPolicy, Options};

use ast_grep_core::language::Language;

pub use combined::{CombinedScan, NO_SUPPRESS_ALL_ID, UNUSED_SUPPRESSION_ID};
pub use fixer::Fixer;
pub use label::{Label, LabelStyle};
pub use rule::DeserializeEnv;
pub use rule::referent_rule::GlobalRules;
pub use rule::{
  ParameterizedUtilError, Rule, RuleSerializeError, SelectorError, SerializableGlobalRule,
  SerializableRule, parse_selector,
};
pub use rule_collection::RuleCollection;
pub use rule_config::{
  Metadata, RuleConfig, RuleConfigError, SerializableRewriter, SerializableRuleConfig, Severity,
};
pub use rule_core::{RuleCore, RuleCoreError, SerializableRuleCore};
pub use transform::Transformation;

fn yaml_options() -> Options {
  let mut options = Options::default();
  // Preserve YAML 1.2 scalars (e.g. Y, on and off are strings), literal merge
  // keys, and non-finite floats in arbitrary rule metadata.
  options.strict_booleans = true;
  options.merge_keys = MergeKeyPolicy::AsOrdinary;
  options.non_finite_float_policy = NonFiniteFloatPolicy::PassThrough;
  options.reject_unsupported_tags = true;
  options
}

pub fn from_str<'de, T: Deserialize<'de>>(s: &'de str) -> Result<T, YamlError> {
  serde_saphyr::from_str_with_options(s, yaml_options())
}

/// Deserialize a YAML document stream using the same options as rule configuration.
/// Null and empty documents are skipped.
pub fn from_multiple<'de, T: Deserialize<'de>>(s: &'de str) -> Result<Vec<T>, YamlError> {
  let mut options = yaml_options();
  if let Some(budget) = &mut options.budget {
    // Rule and test files can contain arbitrarily many documents. Keep the
    // other parser resource limits while avoiding the default 1,024-document cap.
    budget.max_documents = usize::MAX;
  }
  serde_saphyr::from_multiple_with_options(s, options)
}

pub fn from_yaml_string<'a, L: Language + Deserialize<'a>>(
  yamls: &'a str,
  registration: &GlobalRules,
) -> Result<Vec<RuleConfig<L>>, RuleConfigError> {
  let mut ret = vec![];
  for yaml in from_multiple::<SerializableRuleConfig<L>>(yamls)? {
    let config = RuleConfig::try_from(yaml, registration)?;
    ret.push(config);
  }
  Ok(ret)
}
#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_core::Language;
  use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
  use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
  use std::path::Path;

  #[test]
  fn test_yaml_boolean_resolution() {
    use serde_value::Value;
    let values: Vec<Value> = from_str("[Y, N, yes, no, on, off, true, FALSE]").unwrap();
    assert_eq!(
      values,
      vec![
        Value::String("Y".into()),
        Value::String("N".into()),
        Value::String("yes".into()),
        Value::String("no".into()),
        Value::String("on".into()),
        Value::String("off".into()),
        Value::Bool(true),
        Value::Bool(false),
      ]
    );
    assert!(from_str::<bool>("yes").is_err());
  }

  #[test]
  fn test_yaml_document_stream_borrows_and_preserves_scalars() {
    let docs: Vec<&str> = from_multiple("Y\n---\noff").unwrap();
    assert_eq!(docs, ["Y", "off"]);
    assert!(from_multiple::<String>("").unwrap().is_empty());
    assert!(
      from_multiple::<String>("# no documents\n")
        .unwrap()
        .is_empty()
    );
  }

  #[test]
  fn test_yaml_document_stream_skips_null_and_empty_documents() {
    let rule = make_yaml("  pattern: let a = 123");
    for null in [
      "null",
      "~",
      "Null",
      "NULL",
      "---",
      "---\n# empty document\n",
    ] {
      assert!(
        from_yaml_string::<TypeScript>(null, &GlobalRules::default())
          .unwrap()
          .is_empty()
      );
      let stream = format!("{rule}\n---\n{null}\n---\n{rule}");
      let rules = from_yaml_string::<TypeScript>(&stream, &GlobalRules::default()).unwrap();
      assert_eq!(rules.len(), 2);
    }
    let strings: Vec<String> = from_multiple("'null'\n---\n\"~\"").unwrap();
    assert_eq!(strings, ["null", "~"]);
    let nested_null = format!("{rule}metadata:\n  value: null\n");
    assert!(from_yaml_string::<TypeScript>(&nested_null, &GlobalRules::default()).is_ok());
  }

  #[test]
  fn test_yaml_document_stream_over_1024_documents() {
    let yaml = std::iter::repeat_n("Y", 1025)
      .collect::<Vec<_>>()
      .join("\n---\n");
    let docs: Vec<String> = from_multiple(&yaml).unwrap();
    assert_eq!(docs.len(), 1025);
    assert!(docs.iter().all(|doc| doc == "Y"));
  }

  #[derive(Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
  pub enum TypeScript {
    Tsx,
  }
  impl Language for TypeScript {
    fn kind_to_id(&self, kind: &str) -> u16 {
      TSLanguage::from(tree_sitter_typescript::LANGUAGE_TSX).id_for_node_kind(kind, true)
    }
    fn field_to_id(&self, field: &str) -> Option<u16> {
      TSLanguage::from(tree_sitter_typescript::LANGUAGE_TSX)
        .field_id_for_name(field)
        .map(|f| f.get())
    }
    fn from_path<P: AsRef<Path>>(_path: P) -> Option<Self> {
      Some(TypeScript::Tsx)
    }
    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
      builder.build(|src| StrDoc::try_new(src, self.clone()))
    }
  }
  impl LanguageExt for TypeScript {
    fn get_ts_language(&self) -> TSLanguage {
      tree_sitter_typescript::LANGUAGE_TSX.into()
    }
  }

  fn test_rule_match(yaml: &str, source: &str) {
    let globals = GlobalRules::default();
    let config = &from_yaml_string::<TypeScript>(yaml, &globals).expect("rule should parse")[0];
    let grep = config.language.ast_grep(source);
    assert!(grep.root().find(&config.matcher).is_some());
  }

  fn test_rule_unmatch(yaml: &str, source: &str) {
    let globals = GlobalRules::default();
    let config = &from_yaml_string::<TypeScript>(yaml, &globals).expect("rule should parse")[0];
    let grep = config.language.ast_grep(source);
    assert!(grep.root().find(&config.matcher).is_none());
  }

  fn make_yaml(rule: &str) -> String {
    format!(
      r"
id: test
message: test rule
severity: info
language: Tsx
rule:
{rule}
"
    )
  }

  #[test]
  fn test_deserialize_rule_config() {
    let yaml = &make_yaml(
      "
  pattern: let a = 123
",
    );
    test_rule_match(yaml, "let a = 123; let b = 33;");
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "const a = 33");
  }

  #[test]
  fn test_rule_config_deserialize_with_saphyr() {
    let yaml = make_yaml("  pattern: let a = 123");
    let globals = GlobalRules::default();
    let config = serde_saphyr::with_deserializer_from_str_with_options(
      &yaml,
      yaml_options(),
      |deserializer| {
        Ok(RuleConfig::<TypeScript>::deserialize(
          deserializer,
          &globals,
        ))
      },
    )
    .unwrap()
    .unwrap();
    let grep = TypeScript::Tsx.ast_grep("let a = 123;");
    assert!(grep.root().find(&config.matcher).is_some());
  }

  #[test]
  fn test_deserialize_nested() {
    let yaml = &make_yaml(
      "
  all:
    - pattern: let $A = 123
    - pattern: let a = $B
",
    );
    test_rule_match(yaml, "let a = 123; let b = 33;");
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "const a = 33");
    test_rule_unmatch(yaml, "let a = 33");
  }

  #[test]
  fn test_deserialize_kind() {
    let yaml = &make_yaml(
      "
    kind: class_body
",
    );
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "const B = { func() {let a = 123; }}");
  }

  #[test]
  fn test_deserialize_inside() {
    let yaml = &make_yaml(
      "
  all:
    - inside:
        kind: class_body
        stopBy: end
    - pattern: let a = 123
",
    );
    test_rule_unmatch(yaml, "let a = 123; let b = 33;");
    test_rule_match(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "let a = 123");
  }

  #[test]
  fn test_deserialize_not_inside() {
    let yaml = &make_yaml(
      "
  all:
    - not:
        inside:
          kind: class_body
          stopBy: end
    - pattern: let a = 123
",
    );
    test_rule_match(yaml, "let a = 123; let b = 33;");
    test_rule_unmatch(yaml, "class B { func() {let a = 123; }}");
    test_rule_unmatch(yaml, "let a = 13");
  }

  #[test]
  fn test_deserialize_meta_var() {
    let yaml = &make_yaml(
      "
  all:
    - inside:
        any:
          - pattern: function $A($$$) { $$$ }
          - pattern: let $A = ($$$) => $$$
        stopBy: end
    - pattern: $A($$$)
",
    );
    test_rule_match(yaml, "function recursion() { recursion() }");
    test_rule_match(yaml, "let recursion = () => { recursion() }");
    test_rule_unmatch(yaml, "function callOther() { other() }");
  }

  #[test]
  fn test_deserialize_constraints() {
    let yaml = r"
id: test
message: test rule
severity: info
language: Tsx
rule:
  all:
    - pattern: console.log($A)
    - inside:
        pattern: function $B() {$$$}
        stopBy: end
constraints:
  B:
    regex: test
";
    test_rule_match(yaml, "function test() { console.log(1) }");
    test_rule_match(yaml, "function test() { console.log(2) }");
    test_rule_unmatch(yaml, "function tt() { console.log(2) }");
  }

  // https://github.com/ast-grep/ast-grep/issues/813
  #[test]
  fn test_util_rule_with_vaargs() {
    let yaml = r"
id: sibling
language: Tsx
utils:
  utilpat:
    pattern: '$A($$$B);'
rule:
  matches: utilpat
  follows:
    matches: utilpat
    stopBy: end
";
    test_rule_match(yaml, "a();a(123);a();a(123)");
  }
}
