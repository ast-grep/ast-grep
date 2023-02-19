use serde::{Deserialize, Serialize};

use crate::referent_rule::RuleRegistration;
use crate::rule::Rule;
use ast_grep_core::language::Language;
use ast_grep_core::matcher::{KindMatcher, KindMatcherError, RegexMatcher, RegexMatcherError};
use ast_grep_core::meta_var::{MetaVarEnv, MetaVarMatcher, MetaVarMatchers};
use ast_grep_core::{Matcher, Node, Pattern, PatternError};

use bit_set::BitSet;
use thiserror::Error;

use std::collections::HashMap;
use std::ops::Deref;

#[derive(Serialize, Deserialize, Clone)]
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
) -> Result<MetaVarMatcher<L>, SerializeConstraintsError> {
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
) -> Result<MetaVarMatchers<L>, SerializeConstraintsError> {
  let mut map = MetaVarMatchers::new();
  for (key, matcher) in meta_vars {
    map.insert(key, try_from_serializable(matcher, lang.clone())?);
  }
  Ok(map)
}

pub struct RuleWithConstraint<L: Language> {
  rule: Rule<L>,
  matchers: MetaVarMatchers<L>,
  kinds: Option<BitSet>,
  // this is required to hold util rule reference
  utils: RuleRegistration<L>,
}

impl<L: Language> RuleWithConstraint<L> {
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
  pub fn with_matchers(self, matchers: MetaVarMatchers<L>) -> Self {
    Self { matchers, ..self }
  }

  #[inline]
  pub fn with_utils(self, utils: RuleRegistration<L>) -> Self {
    Self { utils, ..self }
  }
}
impl<L: Language> Deref for RuleWithConstraint<L> {
  type Target = Rule<L>;
  fn deref(&self) -> &Self::Target {
    &self.rule
  }
}

impl<L: Language> Default for RuleWithConstraint<L> {
  #[inline]
  fn default() -> Self {
    Self {
      rule: Rule::default(),
      matchers: MetaVarMatchers::default(),
      kinds: None,
      utils: RuleRegistration::default(),
    }
  }
}

impl<L: Language> Matcher<L> for RuleWithConstraint<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    if let Some(kinds) = &self.kinds {
      if !kinds.contains(node.kind_id().into()) {
        return None;
      }
    }
    let ret = self.rule.match_node_with_env(node, env);
    if ret.is_some() && env.match_constraints(&self.matchers) {
      ret
    } else {
      None
    }
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
    let rule = RuleWithConstraint::new(Rule::Pattern(Pattern::new("$A", TypeScript::Tsx)))
      .with_matchers(matchers);
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
