use crate::fixer::{Fixer, FixerError, SerializableFixer};
use crate::rule::referent_rule::RuleRegistration;
use crate::rule::Rule;
use crate::rule::{RuleSerializeError, SerializableRule};
use crate::transform::{apply_env_transform, Transformation};
use crate::DeserializeEnv;
use crate::GlobalRules;

use ast_grep_core::language::Language;
use ast_grep_core::matcher::{KindMatcher, KindMatcherError, RegexMatcher, RegexMatcherError};
use ast_grep_core::meta_var::{MetaVarEnv, MetaVarMatcher, MetaVarMatchers};
use ast_grep_core::replacer::IndentSensitive;
use ast_grep_core::{Doc, Matcher, Node, Pattern, PatternError, StrDoc};
use serde::{Deserialize, Serialize};
use serde_yaml::Error as YamlError;

use bit_set::BitSet;
use schemars::JsonSchema;
use thiserror::Error;

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SerializableMetaVarMatcher {
  /// A regex to filter metavar based on its textual content.
  Regex(String),
  /// A pattern to filter matched metavar based on its AST tree shape.
  Pattern(String),
  /// A kind_id to filter matched metavar based on its ts-node kind
  Kind(String),
}

#[derive(Debug, Error)]
pub enum SerializeConstraintsError {
  #[error("Invalid Regex.")]
  RegexError(#[from] RegexMatcherError),
  #[error("Invalid Kind.")]
  InvalidKind(#[from] KindMatcherError),
  #[error("Invalid Pattern.")]
  PatternError(#[from] PatternError),
}

pub fn try_from_serializable<L: Language>(
  meta_var: SerializableMetaVarMatcher,
  lang: L,
) -> Result<MetaVarMatcher<StrDoc<L>>, SerializeConstraintsError> {
  use SerializableMetaVarMatcher as S;
  Ok(match meta_var {
    S::Regex(s) => MetaVarMatcher::Regex(RegexMatcher::try_new(&s)?),
    S::Kind(p) => MetaVarMatcher::Kind(KindMatcher::try_new(&p, lang)?),
    S::Pattern(p) => MetaVarMatcher::Pattern(Pattern::try_new(&p, lang)?),
  })
}

pub fn try_deserialize_matchers<L: Language>(
  meta_vars: HashMap<String, SerializableMetaVarMatcher>,
  lang: L,
) -> Result<MetaVarMatchers<StrDoc<L>>, SerializeConstraintsError> {
  let mut map = MetaVarMatchers::new();
  for (key, matcher) in meta_vars {
    map.insert(key, try_from_serializable(matcher, lang.clone())?);
  }
  Ok(map)
}

#[derive(Debug, Error)]
pub enum RuleConfigError {
  #[error("Fail to parse yaml as RuleConfig")]
  Yaml(#[from] YamlError),
  #[error("Rule is not configured correctly.")]
  Rule(#[from] RuleSerializeError),
  #[error("fix pattern is invalid.")]
  Fixer(#[from] FixerError),
  #[error("constraints is not configured correctly.")]
  Constraints(#[from] SerializeConstraintsError),
}

type RResult<T> = std::result::Result<T, RuleConfigError>;

/// Used for global rules, rewriters, and pyo3/napi
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRuleCore<L: Language> {
  /// Specify the language to parse and the file extension to include in matching.
  pub language: L,
  /// A rule object to find matching AST nodes
  pub rule: SerializableRule,
  /// Additional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
  /// Utility rules that can be used in `matches`
  pub utils: Option<HashMap<String, SerializableRule>>,
  /// A dictionary for metavariable manipulation. Dict key is the new variable name.
  /// Dict value is a [transformation] that specifies how meta var is processed.
  /// See [transformation doc](https://ast-grep.github.io/reference/yaml/transformation.html).
  pub transform: Option<HashMap<String, Transformation>>,
  /// A pattern string or a FixConfig object to auto fix the issue.
  /// It can reference metavariables appeared in rule.
  /// See details in fix [object reference](https://ast-grep.github.io/reference/yaml/fix.html#fixconfig).
  pub fix: Option<SerializableFixer>,
}

impl<L: Language> SerializableRuleCore<L> {
  pub fn get_deserialize_env(&self, globals: &GlobalRules<L>) -> RResult<DeserializeEnv<L>> {
    let env = DeserializeEnv::new(self.language.clone()).with_globals(globals);
    if let Some(utils) = &self.utils {
      let env = env.register_local_utils(utils)?;
      Ok(env)
    } else {
      Ok(env)
    }
  }

  fn get_meta_var_matchers(&self) -> RResult<MetaVarMatchers<StrDoc<L>>> {
    Ok(if let Some(constraints) = self.constraints.clone() {
      try_deserialize_matchers(constraints, self.language.clone())?
    } else {
      MetaVarMatchers::default()
    })
  }

  fn get_fixer<C: IndentSensitive>(&self, env: &DeserializeEnv<L>) -> RResult<Option<Fixer<C, L>>> {
    if let Some(fix) = &self.fix {
      let parsed = Fixer::parse(fix, env, &self.transform)?;
      Ok(Some(parsed))
    } else {
      Ok(None)
    }
  }

  pub(crate) fn get_matcher_from_env(&self, env: &DeserializeEnv<L>) -> RResult<RuleCore<L>> {
    let rule = env.deserialize_rule(self.rule.clone())?;
    let matchers = self.get_meta_var_matchers()?;
    let transform = self.transform.clone();
    let fixer = self.get_fixer(env)?;
    Ok(
      RuleCore::new(rule)
        .with_matchers(matchers)
        .with_utils(env.registration.clone())
        .with_transform(transform)
        .with_fixer(fixer),
    )
  }

  pub fn get_matcher(&self, globals: &GlobalRules<L>) -> RResult<RuleCore<L>> {
    let env = self.get_deserialize_env(globals)?;
    self.get_matcher_from_env(&env)
  }
}

pub struct RuleCore<L: Language> {
  rule: Rule<L>,
  matchers: MetaVarMatchers<StrDoc<L>>,
  kinds: Option<BitSet>,
  transform: Option<HashMap<String, Transformation>>,
  pub fixer: Option<Fixer<String, L>>,
  // this is required to hold util rule reference
  utils: RuleRegistration<L>,
}

impl<L: Language> RuleCore<L> {
  #[inline]
  pub fn new(rule: Rule<L>) -> Self {
    let kinds = rule.potential_kinds();
    Self {
      rule,
      kinds,
      ..Default::default()
    }
  }

  #[inline]
  pub fn with_matchers(self, matchers: MetaVarMatchers<StrDoc<L>>) -> Self {
    Self { matchers, ..self }
  }

  #[inline]
  pub fn with_utils(self, utils: RuleRegistration<L>) -> Self {
    Self { utils, ..self }
  }

  #[inline]
  pub fn with_transform(self, transform: Option<HashMap<String, Transformation>>) -> Self {
    Self { transform, ..self }
  }

  #[inline]
  pub fn with_fixer(self, fixer: Option<Fixer<String, L>>) -> Self {
    Self { fixer, ..self }
  }

  pub fn get_env(&self, lang: L) -> DeserializeEnv<L> {
    DeserializeEnv {
      lang,
      registration: self.utils.clone(),
    }
  }

  pub fn add_rewrites(
    &mut self,
    rewrites: HashMap<String, (RuleCore<L>, SerializableFixer)>,
  ) -> RResult<()> {
    for (id, rewrite) in rewrites {
      self
        .utils
        .insert_rewrite(&id, rewrite)
        .map_err(RuleSerializeError::from)?;
    }
    Ok(())
  }
}
impl<L: Language> Deref for RuleCore<L> {
  type Target = Rule<L>;
  fn deref(&self) -> &Self::Target {
    &self.rule
  }
}

impl<L: Language> Default for RuleCore<L> {
  #[inline]
  fn default() -> Self {
    Self {
      rule: Rule::default(),
      matchers: MetaVarMatchers::default(),
      kinds: None,
      transform: None,
      fixer: None,
      utils: RuleRegistration::default(),
    }
  }
}

impl<L: Language> Matcher<L> for RuleCore<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(kinds) = &self.kinds {
      if !kinds.contains(node.kind_id().into()) {
        return None;
      }
    }
    let ret = self.rule.match_node_with_env(node, env)?;
    if !env.match_constraints(&self.matchers) {
      return None;
    }
    if let Some(trans) = &self.transform {
      let lang = ret.lang();
      let rewriters = self.utils.get_rewrites();
      apply_env_transform(trans, lang, env.to_mut(), &*rewriters);
    }
    Some(ret)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    self.rule.potential_kinds()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;

  macro_rules! cast {
    ($reg: expr, $pattern: path) => {
      match $reg {
        $pattern(a) => a,
        _ => panic!("non-matching variant"),
      }
    };
  }

  #[test]
  fn test_rule_with_constraints() {
    let mut matchers = MetaVarMatchers::new();
    matchers.insert(
      "A".to_string(),
      MetaVarMatcher::Regex(RegexMatcher::try_new("a").unwrap()),
    );
    let rule =
      RuleCore::new(Rule::Pattern(Pattern::new("$A", TypeScript::Tsx))).with_matchers(matchers);
    let grep = TypeScript::Tsx.ast_grep("a");
    assert!(grep.root().find(&rule).is_some());
    let grep = TypeScript::Tsx.ast_grep("bbb");
    assert!(grep.root().find(&rule).is_none());
  }

  #[test]
  fn test_serializable_regex() {
    let yaml = from_str("regex: aa").expect("must parse");
    let matcher = try_from_serializable(yaml, TypeScript::Tsx).expect("should parse");
    let reg = cast!(matcher, MetaVarMatcher::Regex);
    let matched = TypeScript::Tsx.ast_grep("var aa = 1");
    assert!(matched.root().find(&reg).is_some());
    let non_matched = TypeScript::Tsx.ast_grep("var b = 2");
    assert!(non_matched.root().find(&reg).is_none());
  }

  #[test]
  fn test_non_serializable_regex() {
    let yaml = from_str("regex: '*'").expect("must parse");
    let matcher = try_from_serializable(yaml, TypeScript::Tsx);
    assert!(matches!(
      matcher,
      Err(SerializeConstraintsError::RegexError(_))
    ));
  }

  // TODO: test invalid pattern
  #[test]
  fn test_serializable_pattern() {
    let yaml = from_str("pattern: var a = 1").expect("must parse");
    let matcher = try_from_serializable(yaml, TypeScript::Tsx).expect("should parse");
    let pattern = cast!(matcher, MetaVarMatcher::Pattern);
    let matched = TypeScript::Tsx.ast_grep("var a = 1");
    assert!(matched.root().find(&pattern).is_some());
    let non_matched = TypeScript::Tsx.ast_grep("var b = 2");
    assert!(non_matched.root().find(&pattern).is_none());
  }

  #[test]
  fn test_serializable_kind() {
    let yaml = from_str("kind: class_body").expect("must parse");
    let matcher = try_from_serializable(yaml, TypeScript::Tsx).expect("should parse");
    let pattern = cast!(matcher, MetaVarMatcher::Kind);
    let matched = TypeScript::Tsx.ast_grep("class A {}");
    assert!(matched.root().find(&pattern).is_some());
    let non_matched = TypeScript::Tsx.ast_grep("function b() {}");
    assert!(non_matched.root().find(&pattern).is_none());
  }

  #[test]
  fn test_non_serializable_kind() {
    let yaml = from_str("kind: IMPOSSIBLE_KIND").expect("must parse");
    let matcher = try_from_serializable(yaml, TypeScript::Tsx);
    let error = match matcher {
      Err(SerializeConstraintsError::InvalidKind(s)) => s,
      _ => panic!("serialization should fail for invalid kind"),
    };
    assert_eq!(error.to_string(), "Kind `IMPOSSIBLE_KIND` is invalid.");
  }
}
