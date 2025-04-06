use crate::GlobalRules;

use crate::check_var::{check_rewriters_in_transform, CheckHint};
use crate::fixer::Fixer;
use crate::rule::DeserializeEnv;
use crate::rule_core::{RuleCore, RuleCoreError, SerializableRuleCore};

use ast_grep_core::language::Language;
use ast_grep_core::replacer::Replacer;
use ast_grep_core::{Matcher, NodeMatch, StrDoc};

use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_yaml::Error as YamlError;
use serde_yaml::{with::singleton_map_recursive::deserialize, Deserializer};
use thiserror::Error;

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

#[derive(Serialize, Deserialize, Clone, Default, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
  #[default]
  /// A kind reminder for code with potential improvement.
  Hint,
  /// A suggestion that code can be improved or optimized.
  Info,
  /// A warning that code might produce bugs or does not follow best practice.
  Warning,
  /// An error that code produces bugs or has logic errors.
  Error,
  /// Turns off the rule.
  Off,
}

#[derive(Debug, Error)]
pub enum RuleConfigError {
  #[error("Fail to parse yaml as RuleConfig")]
  Yaml(#[from] YamlError),
  #[error("Fail to parse yaml as Rule.")]
  Core(#[from] RuleCoreError),
  #[error("Rewriter rule `{1}` is not configured correctly.")]
  Rewriter(#[source] RuleCoreError, String),
  #[error("Undefined rewriter `{0}` used in transform.")]
  UndefinedRewriter(String),
  #[error("Rewriter rule `{0}` should have `fix`.")]
  NoFixInRewriter(String),
  #[error("Rule must specify a set of AST kinds to match. Try adding `kind` rule.")]
  MissingPotentialKinds,
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRewriter {
  #[serde(flatten)]
  pub core: SerializableRuleCore,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRuleConfig<L: Language> {
  #[serde(flatten)]
  pub core: SerializableRuleCore,
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Specify the language to parse and the file extension to include in matching.
  pub language: L,
  /// Rewrite rules for `rewrite` transformation
  pub rewriters: Option<Vec<SerializableRewriter>>,
  /// Main message highlighting why this rule fired. It should be single line and concise,
  /// but specific enough to be understood without additional context.
  #[serde(default)]
  pub message: String,
  /// Additional notes to elaborate the message and provide potential fix to the issue.
  pub note: Option<String>,
  /// One of: hint, info, warning, or error
  #[serde(default)]
  pub severity: Severity,
  /// Glob patterns to specify that the rule only applies to matching files
  pub files: Option<Vec<String>>,
  /// Glob patterns that exclude rules from applying to files
  pub ignores: Option<Vec<String>>,
  /// Documentation link to this rule
  pub url: Option<String>,
  /// Extra information for the rule
  pub metadata: Option<Metadata>,
}

/// A trivial wrapper around a HashMap to work around
/// the limitation of `serde_yaml::Value` not implementing `JsonSchema`.
#[derive(Serialize, Deserialize, Clone)]
pub struct Metadata(HashMap<String, serde_yaml::Value>);

impl JsonSchema for Metadata {
  fn schema_name() -> String {
    "Metadata".to_string()
  }
  fn schema_id() -> Cow<'static, str> {
    Cow::Borrowed("Metadata")
  }
  fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
    use schemars::schema::{InstanceType, ObjectValidation, SchemaObject};
    let subschema = Schema::Bool(true);
    SchemaObject {
      instance_type: Some(InstanceType::Object.into()),
      object: Some(Box::new(ObjectValidation {
        additional_properties: Some(Box::new(subschema)),
        ..Default::default()
      })),
      ..Default::default()
    }
    .into()
  }
}

impl<L: Language> SerializableRuleConfig<L> {
  pub fn get_matcher(&self, globals: &GlobalRules<L>) -> Result<RuleCore<L>, RuleConfigError> {
    // every RuleConfig has one rewriters, and the rewriter is shared between sub-rules
    // all RuleConfigs has one common globals
    // every sub-rule has one util
    let env = DeserializeEnv::new(self.language.clone()).with_globals(globals);
    let rule = self.core.get_matcher(env.clone())?;
    self.register_rewriters(&rule, env)?;
    Ok(rule)
  }

  fn register_rewriters(
    &self,
    rule: &RuleCore<L>,
    env: DeserializeEnv<L>,
  ) -> Result<(), RuleConfigError> {
    let Some(ser) = &self.rewriters else {
      return Ok(());
    };
    let reg = &env.registration;
    let vars = rule.defined_vars();
    for val in ser {
      if val.core.fix.is_none() {
        return Err(RuleConfigError::NoFixInRewriter(val.id.clone()));
      }
      let rewriter = val
        .core
        .get_matcher_with_hint(env.clone(), CheckHint::Rewriter(&vars))
        .map_err(|e| RuleConfigError::Rewriter(e, val.id.clone()))?;
      reg.insert_rewriter(&val.id, rewriter);
    }
    check_rewriters_in_transform(rule, reg.get_rewriters())?;
    Ok(())
  }
}

impl<L: Language> Deref for SerializableRuleConfig<L> {
  type Target = SerializableRuleCore;
  fn deref(&self) -> &Self::Target {
    &self.core
  }
}

impl<L: Language> DerefMut for SerializableRuleConfig<L> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.core
  }
}

pub struct RuleConfig<L: Language> {
  inner: SerializableRuleConfig<L>,
  pub matcher: RuleCore<L>,
}

impl<L: Language> RuleConfig<L> {
  pub fn try_from(
    inner: SerializableRuleConfig<L>,
    globals: &GlobalRules<L>,
  ) -> Result<Self, RuleConfigError> {
    let matcher = inner.get_matcher(globals)?;
    if matcher.potential_kinds().is_none() {
      return Err(RuleConfigError::MissingPotentialKinds);
    }
    Ok(Self { inner, matcher })
  }

  pub fn deserialize<'de>(
    deserializer: Deserializer<'de>,
    globals: &GlobalRules<L>,
  ) -> Result<Self, RuleConfigError>
  where
    L: Deserialize<'de>,
  {
    let inner: SerializableRuleConfig<L> = deserialize(deserializer)?;
    Self::try_from(inner, globals)
  }

  pub fn get_message(&self, node: &NodeMatch<StrDoc<L>>) -> String {
    let env = self.matcher.get_env(self.language.clone());
    let parsed = Fixer::with_transform(&self.message, &env, &self.transform).expect("should work");
    let bytes = parsed.generate_replacement(node);
    String::from_utf8(bytes).expect("replacement must be valid utf-8")
  }
  pub fn get_fixer(&self) -> Result<Option<Fixer<L>>, RuleConfigError> {
    if let Some(fix) = &self.fix {
      let env = self.matcher.get_env(self.language.clone());
      let parsed = Fixer::parse(fix, &env, &self.transform).map_err(RuleCoreError::Fixer)?;
      Ok(Some(parsed))
    } else {
      Ok(None)
    }
  }
}
impl<L: Language> Deref for RuleConfig<L> {
  type Target = SerializableRuleConfig<L>;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<L: Language> DerefMut for RuleConfig<L> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::rule::SerializableRule;
  use crate::test::TypeScript;

  fn ts_rule_config(rule: SerializableRule) -> SerializableRuleConfig<TypeScript> {
    let core = SerializableRuleCore {
      rule,
      constraints: None,
      transform: None,
      utils: None,
      fix: None,
    };
    SerializableRuleConfig {
      core,
      id: "".into(),
      language: TypeScript::Tsx,
      rewriters: None,
      message: "".into(),
      note: None,
      severity: Severity::Hint,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }

  #[test]
  fn test_rule_message() {
    let globals = GlobalRules::default();
    let rule = from_str("pattern: class $A {}").expect("cannot parse rule");
    let mut config = ts_rule_config(rule);
    config.id = "test".into();
    config.message = "Found $A".into();
    let config = RuleConfig::try_from(config, &Default::default()).expect("should work");
    let grep = TypeScript::Tsx.ast_grep("class TestClass {}");
    let node_match = grep
      .root()
      .find(config.get_matcher(&globals).unwrap())
      .expect("should find match");
    assert_eq!(config.get_message(&node_match), "Found TestClass");
  }

  #[test]
  fn test_augmented_rule() {
    let globals = GlobalRules::default();
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  stopBy: end
  pattern: function test() { $$$ }
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("console.log(1)");
    let matcher = config.get_matcher(&globals).unwrap();
    assert!(grep.root().find(&matcher).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    assert!(grep.root().find(&matcher).is_some());
  }

  #[test]
  fn test_multiple_augment_rule() {
    let globals = GlobalRules::default();
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  stopBy: end
  pattern: function test() { $$$ }
has:
  stopBy: end
  pattern: '123'
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    let matcher = config.get_matcher(&globals).unwrap();
    assert!(grep.root().find(&matcher).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(123) }");
    assert!(grep.root().find(&matcher).is_some());
  }

  #[test]
  fn test_rule_env() {
    let globals = GlobalRules::default();
    let rule = from_str(
      "
all:
  - pattern: console.log($A)
  - inside:
      stopBy: end
      pattern: function $B() {$$$}
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    let node_match = grep
      .root()
      .find(config.get_matcher(&globals).unwrap())
      .expect("should found");
    let env = node_match.get_env();
    let a = env.get_match("A").expect("should exist").text();
    assert_eq!(a, "1");
    let b = env.get_match("B").expect("should exist").text();
    assert_eq!(b, "test");
  }

  #[test]
  fn test_transform() {
    let globals = GlobalRules::default();
    let rule = from_str("pattern: console.log($A)").expect("should parse");
    let mut config = ts_rule_config(rule);
    let transform = from_str(
      "
B:
  substring:
    source: $A
    startChar: 1
    endChar: -1
",
    )
    .expect("should parse");
    config.transform = Some(transform);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(123) }");
    let node_match = grep
      .root()
      .find(config.get_matcher(&globals).unwrap())
      .expect("should found");
    let env = node_match.get_env();
    let a = env.get_match("A").expect("should exist").text();
    assert_eq!(a, "123");
    let b = env.get_transformed("B").expect("should exist");
    assert_eq!(b, b"2");
  }

  fn get_matches_config() -> SerializableRuleConfig<TypeScript> {
    let rule = from_str(
      "
matches: test-rule
",
    )
    .unwrap();
    let utils = from_str(
      "
test-rule:
  pattern: some($A)
",
    )
    .unwrap();
    let mut ret = ts_rule_config(rule);
    ret.utils = Some(utils);
    ret
  }

  #[test]
  fn test_utils_rule() {
    let globals = GlobalRules::default();
    let config = get_matches_config();
    let matcher = config.get_matcher(&globals).unwrap();
    let grep = TypeScript::Tsx.ast_grep("some(123)");
    assert!(grep.root().find(&matcher).is_some());
    let grep = TypeScript::Tsx.ast_grep("some()");
    assert!(grep.root().find(&matcher).is_none());
  }
  #[test]
  fn test_get_fixer() {
    let globals = GlobalRules::default();
    let mut config = get_matches_config();
    config.fix = Some(from_str("string!!").unwrap());
    let rule = RuleConfig::try_from(config, &globals).unwrap();
    let fixer = rule.get_fixer().unwrap().unwrap();
    let grep = TypeScript::Tsx.ast_grep("some(123)");
    let nm = grep.root().find(&rule.matcher).unwrap();
    let replacement = fixer.generate_replacement(&nm);
    assert_eq!(String::from_utf8_lossy(&replacement), "string!!");
  }

  #[test]
  fn test_add_rewriters() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
transform:
  B:
    rewrite:
      rewriters: [re]
      source: $A
rewriters:
- id: re
  rule: {kind: number}
  fix: yjsnp
    ",
    )
    .expect("should parse");
    let rule = RuleConfig::try_from(rule, &Default::default()).expect("work");
    let grep = TypeScript::Tsx.ast_grep("a = 123");
    let nm = grep.root().find(&rule.matcher).unwrap();
    let b = nm.get_env().get_transformed("B").expect("should have");
    assert_eq!(String::from_utf8_lossy(b), "yjsnp");
  }

  #[test]
  fn test_rewriters_access_utils() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
utils:
  num: { kind: number }
transform:
  B:
    rewrite:
      rewriters: [re]
      source: $A
rewriters:
- id: re
  rule: {matches: num, pattern: $NOT}
  fix: yjsnp
    ",
    )
    .expect("should parse");
    let rule = RuleConfig::try_from(rule, &Default::default()).expect("work");
    let grep = TypeScript::Tsx.ast_grep("a = 456");
    let nm = grep.root().find(&rule.matcher).unwrap();
    let b = nm.get_env().get_transformed("B").expect("should have");
    assert!(nm.get_env().get_match("NOT").is_none());
    assert_eq!(String::from_utf8_lossy(b), "yjsnp");
  }

  #[test]
  fn test_rewriter_utils_should_not_pollute_registration() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {matches: num}
language: Tsx
transform:
  B:
    rewrite:
      rewriters: [re]
      source: $B
rewriters:
- id: re
  rule: {matches: num}
  fix: yjsnp
  utils:
    num: { kind: number }
    ",
    )
    .expect("should parse");
    let ret = RuleConfig::try_from(rule, &Default::default());
    assert!(matches!(ret, Err(RuleConfigError::Core(_))));
  }

  #[test]
  fn test_rewriter_should_have_fix() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {kind: number}
language: Tsx
rewriters:
- id: wrong
  rule: {matches: num}",
    )
    .expect("should parse");
    let ret = RuleConfig::try_from(rule, &Default::default());
    match ret {
      Err(RuleConfigError::NoFixInRewriter(name)) => assert_eq!(name, "wrong"),
      _ => panic!("unexpected error"),
    }
  }

  #[test]
  fn test_utils_in_rewriter_should_work() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
transform:
  B:
    rewrite:
      rewriters: [re]
      source: $A
rewriters:
- id: re
  rule: {matches: num}
  fix: yjsnp
  utils:
    num: { kind: number }
    ",
    )
    .expect("should parse");
    let rule = RuleConfig::try_from(rule, &Default::default()).expect("work");
    let grep = TypeScript::Tsx.ast_grep("a = 114514");
    let nm = grep.root().find(&rule.matcher).unwrap();
    let b = nm.get_env().get_transformed("B").expect("should have");
    assert_eq!(String::from_utf8_lossy(b), "yjsnp");
  }

  #[test]
  fn test_use_rewriter_recursive() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
transform:
  B: { rewrite: { rewriters: [re], source: $A } }
rewriters:
- id: handle-num
  rule: {regex: '114'}
  fix: '1919810'
- id: re
  rule: {kind: number, pattern: $A}
  transform:
    B: { rewrite: { rewriters: [handle-num], source: $A } }
  fix: $B
    ",
    )
    .expect("should parse");
    let rule = RuleConfig::try_from(rule, &Default::default()).expect("work");
    let grep = TypeScript::Tsx.ast_grep("a = 114514");
    let nm = grep.root().find(&rule.matcher).unwrap();
    let b = nm.get_env().get_transformed("B").expect("should have");
    assert_eq!(String::from_utf8_lossy(b), "1919810");
  }

  fn make_undefined_error(src: &str) -> String {
    let rule: SerializableRuleConfig<TypeScript> = from_str(src).expect("should parse");
    let err = RuleConfig::try_from(rule, &Default::default());
    match err {
      Err(RuleConfigError::UndefinedRewriter(name)) => name,
      _ => panic!("unexpected parsing result"),
    }
  }

  #[test]
  fn test_undefined_rewriter() {
    let undefined = make_undefined_error(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
transform:
  B: { rewrite: { rewriters: [not-defined], source: $A } }
rewriters:
- id: re
  rule: {kind: number, pattern: $A}
  fix: hah
    ",
    );
    assert_eq!(undefined, "not-defined");
  }
  #[test]
  fn test_wrong_rewriter() {
    let rule: SerializableRuleConfig<TypeScript> = from_str(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
rewriters:
- id: wrong
  rule: {kind: '114'}
  fix: '1919810'
    ",
    )
    .expect("should parse");
    let ret = RuleConfig::try_from(rule, &Default::default());
    match ret {
      Err(RuleConfigError::Rewriter(_, name)) => assert_eq!(name, "wrong"),
      _ => panic!("unexpected error"),
    }
  }

  #[test]
  fn test_undefined_rewriter_in_transform() {
    let undefined = make_undefined_error(
      r"
id: test
rule: {pattern: 'a = $A'}
language: Tsx
transform:
  B: { rewrite: { rewriters: [re], source: $A } }
rewriters:
- id: re
  rule: {kind: number, pattern: $A}
  transform:
    C: { rewrite: { rewriters: [nested-undefined], source: $A } }
  fix: hah
    ",
    );
    assert_eq!(undefined, "nested-undefined");
  }

  #[test]
  fn test_rewriter_use_upper_var() {
    let src = r"
id: test
rule: {pattern: '$B = $A'}
language: Tsx
transform:
  D: { rewrite: { rewriters: [re], source: $A } }
rewriters:
- id: re
  rule: {kind: number, pattern: $C}
  fix: $B.$C
    ";
    let rule: SerializableRuleConfig<TypeScript> = from_str(src).expect("should parse");
    let ret = RuleConfig::try_from(rule, &Default::default());
    assert!(ret.is_ok());
  }

  #[test]
  fn test_rewriter_use_undefined_var() {
    let src = r"
id: test
rule: {pattern: '$B = $A'}
language: Tsx
transform:
  B: { rewrite: { rewriters: [re], source: $A } }
rewriters:
- id: re
  rule: {kind: number, pattern: $C}
  fix: $D.$C
    ";
    let rule: SerializableRuleConfig<TypeScript> = from_str(src).expect("should parse");
    let ret = RuleConfig::try_from(rule, &Default::default());
    assert!(ret.is_err());
  }

  #[test]
  fn test_get_message_transform() {
    let src = r"
id: test-rule
language: Tsx
rule: { kind: string, pattern: $ARG }
transform:
  TEST: { replace: { replace: 'a', by: 'b', source: $ARG, } }
message: $TEST
    ";
    let rule: SerializableRuleConfig<TypeScript> = from_str(src).expect("should parse");
    let rule = RuleConfig::try_from(rule, &Default::default()).expect("should work");
    let grep = TypeScript::Tsx.ast_grep("a = '123'");
    let nm = grep.root().find(&rule.matcher).unwrap();
    assert_eq!(rule.get_message(&nm), "'123'");
  }

  #[test]
  fn test_complex_metadata() {
    let src = r"
id: test-rule
language: Tsx
rule: { kind: string }
metadata:
  test: [1, 2, 3]
  ";
    let rule: SerializableRuleConfig<TypeScript> = from_str(src).expect("should parse");
    let rule = RuleConfig::try_from(rule, &Default::default()).expect("should work");
    let grep = TypeScript::Tsx.ast_grep("a = '123'");
    let nm = grep.root().find(&rule.matcher);
    assert!(nm.is_some());
  }
}
