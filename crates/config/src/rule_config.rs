use crate::relational_rule::{Follows, Has, Inside, Precedes};
use crate::serialized_rule::{
  AtomicRule, Augmentation, CompositeRule, PatternStyle, RelationalRule, SerializableRule,
};

pub use crate::constraints::{
  try_deserialize_matchers, try_from_serializable as deserialize_meta_var, RuleWithConstraint,
  SerializableMetaVarMatcher,
};
use ast_grep_core::language::Language;
use ast_grep_core::matcher::{KindMatcher, KindMatcherError};
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::ops as o;
use ast_grep_core::replace_meta_var_in_string;
use ast_grep_core::NodeMatch;
use ast_grep_core::{Matcher, Node, Pattern, PatternError};
use bit_set::BitSet;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
  Hint,
  Info,
  Warning,
  Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RuleConfig<L: Language> {
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Main message highlighting why this rule fired. It should be single line and concise,
  /// but specific enough to be understood without additional context.
  pub message: String,
  /// Additional notes to elaborate the message and provide potential fix to the issue.
  pub note: Option<String>,
  /// One of: Info, Warning, or Error
  pub severity: Severity,
  /// Specify the language to parse and the file extension to includ in matching.
  pub language: L,
  /// Pattern rules to find matching AST nodes
  pub rule: SerializableRule,
  /// A pattern to auto fix the issue. It can reference metavariables appeared in rule.
  pub fix: Option<String>,
  /// Addtional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
  /// Glob patterns to specify that the rule only applies to matching files
  pub files: Option<Vec<String>>,
  /// Glob patterns that exclude rules from applying to files
  pub ignores: Option<Vec<String>>,
  /// Documentation link to this rule
  pub url: Option<String>,
  /// Extra information for the rule
  pub metadata: Option<HashMap<String, String>>,
}

impl<L: Language> RuleConfig<L> {
  pub fn get_matcher(&self) -> RuleWithConstraint<L> {
    let rule = self.get_rule();
    let matchers = self.get_meta_var_matchers();
    RuleWithConstraint { rule, matchers }
  }

  fn get_rule(&self) -> Rule<L> {
    // TODO
    try_from_serializable(self.rule.clone(), self.language.clone()).unwrap()
  }

  pub fn get_fixer(&self) -> Option<Pattern<L>> {
    // TODO
    Some(Pattern::try_new(self.fix.as_ref()?, self.language.clone()).unwrap())
  }

  pub fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
    if let Some(constraints) = self.constraints.clone() {
      try_deserialize_matchers(constraints, self.language.clone()).unwrap()
    } else {
      MetaVarMatchers::default()
    }
  }

  pub fn get_message(&self, node: &NodeMatch<L>) -> String {
    replace_meta_var_in_string(&self.message, node.get_env(), node.lang())
  }
}

pub enum Rule<L: Language> {
  All(o::All<L, Rule<L>>),
  Any(o::Any<L, Rule<L>>),
  Not(Box<o::Not<L, Rule<L>>>),
  Inside(Box<Inside<L>>),
  Has(Box<Has<L>>),
  Precedes(Box<Precedes<L>>),
  Follows(Box<Follows<L>>),
  Pattern(Pattern<L>),
  Kind(KindMatcher<L>),
}

impl<L: Language> Matcher<L> for Rule<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    use Rule::*;
    match self {
      All(all) => all.match_node_with_env(node, env),
      Any(any) => any.match_node_with_env(node, env),
      Not(not) => not.match_node_with_env(node, env),
      Inside(parent) => match_and_add_label(&**parent, node, env),
      Has(child) => match_and_add_label(&**child, node, env),
      Precedes(latter) => match_and_add_label(&**latter, node, env),
      Follows(former) => match_and_add_label(&**former, node, env),
      Pattern(pattern) => pattern.match_node_with_env(node, env),
      Kind(kind) => kind.match_node_with_env(node, env),
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    use Rule::*;
    match self {
      All(all) => all.potential_kinds(),
      Any(any) => any.potential_kinds(),
      Not(not) => not.potential_kinds(),
      Inside(parent) => parent.potential_kinds(),
      Has(child) => child.potential_kinds(),
      Precedes(latter) => latter.potential_kinds(),
      Follows(former) => former.potential_kinds(),
      Pattern(pattern) => pattern.potential_kinds(),
      Kind(kind) => kind.potential_kinds(),
    }
  }
}

/// Rule matches nothing by default.
/// In Math jargon, Rule is vacuously false.
impl<L: Language> Default for Rule<L> {
  fn default() -> Self {
    Self::Any(o::Any::new(std::iter::empty()))
  }
}

fn match_and_add_label<'tree, L: Language, M: Matcher<L>>(
  inner: &M,
  node: Node<'tree, L>,
  env: &mut MetaVarEnv<'tree, L>,
) -> Option<Node<'tree, L>> {
  let matched = inner.match_node_with_env(node, env)?;
  env.add_label("secondary", matched.clone());
  Some(matched)
}

#[derive(Debug, Error)]
pub enum SerializeError {
  #[error("Rule must have one positive matcher.")]
  MissPositiveMatcher,
  #[error("Rule contains invalid kind matcher.")]
  InvalidKind(#[from] KindMatcherError),
  #[error("Rule contains invalid pattern matcher.")]
  InvalidPattern(#[from] PatternError),
}

// TODO: implement positive/non positive
pub fn try_from_serializable<L: Language>(
  serialized: SerializableRule,
  lang: L,
) -> Result<Rule<L>, SerializeError> {
  use SerializableRule as S;
  match serialized {
    S::Composite(comp) => deserialze_composite_rule(comp, lang),
    S::Relational(relational) => deserialize_relational_rule(relational, lang),
    S::Atomic { rule, augmentation } => deserialze_augmented_atomic_rule(rule, augmentation, lang),
  }
}

fn deserialze_composite_rule<L: Language>(
  composite: CompositeRule,
  lang: L,
) -> Result<Rule<L>, SerializeError> {
  use CompositeRule as C;
  use Rule as R;
  let convert_rules = |rules: Vec<SerializableRule>| -> Result<_, SerializeError> {
    let mut inner = Vec::with_capacity(rules.len());
    for rule in rules {
      inner.push(try_from_serializable(rule, lang.clone())?);
    }
    Ok(inner)
  };
  Ok(match composite {
    C::All(all) => R::All(o::All::new(convert_rules(all)?)),
    C::Any(any) => R::Any(o::Any::new(convert_rules(any)?)),
    C::Not(not) => R::Not(Box::new(o::Not::new(try_from_serializable(*not, lang)?))),
  })
}

fn deserialize_relational_rule<L: Language>(
  relational: RelationalRule,
  lang: L,
) -> Result<Rule<L>, SerializeError> {
  use RelationalRule as RR;
  use Rule as R;
  Ok(match relational {
    RR::Inside(inside) => R::Inside(Box::new(Inside::try_new(*inside, lang)?)),
    RR::Has(has) => R::Has(Box::new(Has::try_new(*has, lang)?)),
    RR::Precedes(precedes) => R::Precedes(Box::new(Precedes::try_new(*precedes, lang)?)),
    RR::Follows(follows) => R::Follows(Box::new(Follows::try_new(*follows, lang)?)),
  })
}

fn deserialze_augmented_atomic_rule<L: Language>(
  rule: AtomicRule,
  augmentation: Augmentation,
  lang: L,
) -> Result<Rule<L>, SerializeError> {
  use AtomicRule as A;
  use Rule as R;
  let l = lang.clone();
  let deserialized_rule = match rule {
    A::Kind(kind) => R::Kind(KindMatcher::try_new(&kind, lang)?),
    A::Pattern(PatternStyle::Str(pattern)) => R::Pattern(Pattern::try_new(&pattern, lang)?),
    A::Pattern(PatternStyle::Contextual { context, selector }) => {
      R::Pattern(Pattern::contextual(&context, &selector, lang)?)
    }
  };
  augment_rule(deserialized_rule, augmentation, l)
}

fn augment_rule<L: Language>(
  rule: Rule<L>,
  aug: Augmentation,
  lang: L,
) -> Result<Rule<L>, SerializeError> {
  let mut rules = vec![];
  use Rule as R;
  if let Some(inside) = aug.inside {
    rules.push(R::Inside(Box::new(Inside::try_new(*inside, lang.clone())?)));
  }
  if let Some(has) = aug.has {
    rules.push(R::Has(Box::new(Has::try_new(*has, lang.clone())?)));
  }
  if let Some(precedes) = aug.precedes {
    rules.push(R::Precedes(Box::new(Precedes::try_new(
      *precedes,
      lang.clone(),
    )?)));
  }
  if let Some(follows) = aug.follows {
    rules.push(R::Follows(Box::new(Follows::try_new(*follows, lang)?)));
  }
  if rules.is_empty() {
    Ok(rule)
  } else {
    rules.push(rule);
    Ok(R::All(o::All::new(rules)))
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;

  fn ts_rule_config(rule: SerializableRule) -> RuleConfig<TypeScript> {
    RuleConfig {
      id: "".into(),
      message: "".into(),
      note: None,
      severity: Severity::Hint,
      language: TypeScript::Tsx,
      rule,
      fix: None,
      constraints: None,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }

  #[test]
  fn test_rule_message() {
    let rule = from_str("pattern: class $A {}").expect("cannot parse rule");
    let config = RuleConfig {
      id: "test".into(),
      message: "Found $A".into(),
      ..ts_rule_config(rule)
    };
    let grep = TypeScript::Tsx.ast_grep("class TestClass {}");
    let node_match = grep
      .root()
      .find(config.get_matcher())
      .expect("should find match");
    assert_eq!(config.get_message(&node_match), "Found TestClass");
  }

  #[test]
  fn test_augmented_rule() {
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  pattern: function test() { $$$ }
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("console.log(1)");
    assert!(grep.root().find(config.get_matcher()).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    assert!(grep.root().find(config.get_matcher()).is_some());
  }

  #[test]
  fn test_multiple_augment_rule() {
    let rule = from_str(
      "
pattern: console.log($A)
inside:
  pattern: function test() { $$$ }
has:
  pattern: '123'
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    assert!(grep.root().find(config.get_matcher()).is_none());
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(123) }");
    assert!(grep.root().find(config.get_matcher()).is_some());
  }

  #[test]
  fn test_rule_env() {
    let rule = from_str(
      "
all:
  - pattern: console.log($A)
  - inside:
      pattern: function $B() {$$$}
",
    )
    .expect("should parse");
    let config = ts_rule_config(rule);
    let grep = TypeScript::Tsx.ast_grep("function test() { console.log(1) }");
    let node_match = grep
      .root()
      .find(config.get_matcher())
      .expect("should found");
    let env = node_match.get_env();
    let a = env.get_match("A").expect("should exist").text();
    assert_eq!(a, "1");
    let b = env.get_match("B").expect("should exist").text();
    assert_eq!(b, "test");
  }
}
