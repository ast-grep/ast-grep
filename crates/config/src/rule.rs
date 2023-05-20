use crate::deserialize_env::DeserializeEnv;
use crate::maybe::Maybe;
use crate::referent_rule::{ReferentRule, ReferentRuleError};
use crate::relational_rule::{Follows, Has, Inside, Precedes, Relation};

use ast_grep_core::language::Language;
use ast_grep_core::matcher::{KindMatcher, KindMatcherError, RegexMatcher, RegexMatcherError};
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::ops as o;
use ast_grep_core::{Doc, Matcher, Node, Pattern as PatternCore, PatternError, StrDoc};

use bit_set::BitSet;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use thiserror::Error;

type Pattern<L> = PatternCore<StrDoc<L>>;

/// We have three kinds of rules in ast-grep.
/// * Atomic: the most basic rule to match AST. We have two variants: Pattern and Kind.
/// * Relational: filter matched target according to their position relative to other nodes.
/// * Composite: use logic operation all/any/not to compose the above rules to larger rules.
/// Every rule has it's unique name so we can combine several rules in one object.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct SerializableRule {
  // avoid embedding AtomicRule/RelationalRule/CompositeRule with flatten here for better error message

  // atomic
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub pattern: Maybe<PatternStyle>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub kind: Maybe<String>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub regex: Maybe<String>,
  // relational
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub inside: Maybe<Box<Relation>>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub has: Maybe<Box<Relation>>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub precedes: Maybe<Box<Relation>>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub follows: Maybe<Box<Relation>>,
  // composite
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub all: Maybe<Vec<SerializableRule>>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub any: Maybe<Vec<SerializableRule>>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub not: Maybe<Box<SerializableRule>>,
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]
  pub matches: Maybe<String>,
}

pub struct Categorized {
  pub atomic: AtomicRule,
  pub relational: RelationalRule,
  pub composite: CompositeRule,
}

impl SerializableRule {
  pub fn categorized(self) -> Categorized {
    Categorized {
      atomic: AtomicRule {
        pattern: self.pattern.into(),
        kind: self.kind.into(),
        regex: self.regex.into(),
      },
      relational: RelationalRule {
        inside: self.inside.into(),
        has: self.has.into(),
        precedes: self.precedes.into(),
        follows: self.follows.into(),
      },
      composite: CompositeRule {
        all: self.all.into(),
        any: self.any.into(),
        not: self.not.into(),
        matches: self.matches.into(),
      },
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AtomicRule {
  pub pattern: Option<PatternStyle>,
  pub kind: Option<String>,
  pub regex: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PatternStyle {
  Str(String),
  Contextual { context: String, selector: String },
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct RelationalRule {
  pub inside: Option<Box<Relation>>,
  pub has: Option<Box<Relation>>,
  pub precedes: Option<Box<Relation>>,
  pub follows: Option<Box<Relation>>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CompositeRule {
  pub all: Option<Vec<SerializableRule>>,
  pub any: Option<Vec<SerializableRule>>,
  pub not: Option<Box<SerializableRule>>,
  pub matches: Option<String>,
}

pub enum Rule<L: Language> {
  // atomic
  Pattern(Pattern<L>),
  Kind(KindMatcher<L>),
  Regex(RegexMatcher<L>),
  // relational
  Inside(Box<Inside<L>>),
  Has(Box<Has<L>>),
  Precedes(Box<Precedes<L>>),
  Follows(Box<Follows<L>>),
  // composite
  All(o::All<L, Rule<L>>),
  Any(o::Any<L, Rule<L>>),
  Not(Box<o::Not<L, Rule<L>>>),
  Matches(ReferentRule<L>),
}
impl<L: Language> Rule<L> {
  pub fn is_atomic(&self) -> bool {
    use Rule::*;
    matches!(self, Pattern(_) | Kind(_) | Regex(_))
  }
  pub fn is_relational(&self) -> bool {
    use Rule::*;
    matches!(self, Inside(_) | Has(_) | Precedes(_) | Follows(_))
  }

  pub fn is_composite(&self) -> bool {
    use Rule::*;
    matches!(self, All(_) | Any(_) | Not(_) | Matches(_))
  }

  pub(crate) fn check_cyclic(&self, id: &str) -> bool {
    match self {
      Rule::All(all) => all.inner().iter().any(|r| r.check_cyclic(id)),
      Rule::Any(any) => any.inner().iter().any(|r| r.check_cyclic(id)),
      Rule::Not(not) => not.inner().check_cyclic(id),
      Rule::Matches(m) => m.rule_id == id,
      rule => {
        debug_assert!(!rule.is_composite());
        false
      }
    }
  }
}

impl<L: Language> Matcher<L> for Rule<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    use Rule::*;
    match self {
      // atomic
      Pattern(pattern) => pattern.match_node_with_env(node, env),
      Kind(kind) => kind.match_node_with_env(node, env),
      Regex(regex) => regex.match_node_with_env(node, env),
      // relational
      Inside(parent) => match_and_add_label(&**parent, node, env),
      Has(child) => match_and_add_label(&**child, node, env),
      Precedes(latter) => match_and_add_label(&**latter, node, env),
      Follows(former) => match_and_add_label(&**former, node, env),
      // composite
      All(all) => all.match_node_with_env(node, env),
      Any(any) => any.match_node_with_env(node, env),
      Not(not) => not.match_node_with_env(node, env),
      Matches(rule) => rule.match_node_with_env(node, env),
    }
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    use Rule::*;
    match self {
      // atomic
      Pattern(pattern) => pattern.potential_kinds(),
      Kind(kind) => kind.potential_kinds(),
      Regex(regex) => regex.potential_kinds(),
      // relational
      Inside(parent) => parent.potential_kinds(),
      Has(child) => child.potential_kinds(),
      Precedes(latter) => latter.potential_kinds(),
      Follows(former) => former.potential_kinds(),
      // composite
      All(all) => all.potential_kinds(),
      Any(any) => any.potential_kinds(),
      Not(not) => not.potential_kinds(),
      Matches(rule) => rule.potential_kinds(),
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

fn match_and_add_label<'tree, D: Doc, M: Matcher<D::Lang>>(
  inner: &M,
  node: Node<'tree, D>,
  env: &mut Cow<MetaVarEnv<'tree, D>>,
) -> Option<Node<'tree, D>> {
  let matched = inner.match_node_with_env(node, env)?;
  env.to_mut().add_label("secondary", matched.clone());
  Some(matched)
}

#[derive(Debug, Error)]
pub enum RuleSerializeError {
  #[error("Rule must have one positive matcher.")]
  MissPositiveMatcher,
  #[error("Rule contains invalid kind matcher.")]
  InvalidKind(#[from] KindMatcherError),
  #[error("Rule contains invalid pattern matcher.")]
  InvalidPattern(#[from] PatternError),
  #[error("Rule contains invalid regex matcher.")]
  WrongRegex(#[from] RegexMatcherError),
  #[error("Rule contains invalid matches reference.")]
  MatchesRefrence(#[from] ReferentRuleError),
  #[error("field is only supported in has/inside.")]
  FieldNotSupported,
}

// TODO: implement positive/non positive
pub fn deserialize_rule<L: Language>(
  serialized: SerializableRule,
  env: &DeserializeEnv<L>,
) -> Result<Rule<L>, RuleSerializeError> {
  let mut rules = Vec::with_capacity(1);
  use Rule as R;
  let categorized = serialized.categorized();
  deserialze_atomic_rule(categorized.atomic, &mut rules, env)?;
  deserialize_relational_rule(categorized.relational, &mut rules, env)?;
  deserialze_composite_rule(categorized.composite, &mut rules, env)?;
  if rules.is_empty() {
    Err(RuleSerializeError::MissPositiveMatcher)
  } else if rules.len() == 1 {
    Ok(rules.pop().expect("should not be empty"))
  } else {
    Ok(R::All(o::All::new(rules)))
  }
}

fn deserialze_composite_rule<L: Language>(
  composite: CompositeRule,
  rules: &mut Vec<Rule<L>>,
  env: &DeserializeEnv<L>,
) -> Result<(), RuleSerializeError> {
  use Rule as R;
  let convert_rules = |rules: Vec<SerializableRule>| -> Result<_, RuleSerializeError> {
    let mut inner = Vec::with_capacity(rules.len());
    for rule in rules {
      inner.push(deserialize_rule(rule, env)?);
    }
    Ok(inner)
  };
  if let Some(all) = composite.all {
    rules.push(R::All(o::All::new(convert_rules(all)?)));
  }
  if let Some(any) = composite.any {
    rules.push(R::Any(o::Any::new(convert_rules(any)?)));
  }
  if let Some(not) = composite.not {
    let not = o::Not::new(deserialize_rule(*not, env)?);
    rules.push(R::Not(Box::new(not)));
  }
  if let Some(id) = composite.matches {
    let matches = ReferentRule::try_new(id, &env.registration)?;
    rules.push(R::Matches(matches));
  }
  Ok(())
}

fn deserialize_relational_rule<L: Language>(
  relational: RelationalRule,
  rules: &mut Vec<Rule<L>>,
  env: &DeserializeEnv<L>,
) -> Result<(), RuleSerializeError> {
  use Rule as R;
  // relational
  if let Some(inside) = relational.inside {
    rules.push(R::Inside(Box::new(Inside::try_new(*inside, env)?)));
  }
  if let Some(has) = relational.has {
    rules.push(R::Has(Box::new(Has::try_new(*has, env)?)));
  }
  if let Some(precedes) = relational.precedes {
    rules.push(R::Precedes(Box::new(Precedes::try_new(*precedes, env)?)));
  }
  if let Some(follows) = relational.follows {
    rules.push(R::Follows(Box::new(Follows::try_new(*follows, env)?)));
  }
  Ok(())
}

fn deserialze_atomic_rule<L: Language>(
  atomic: AtomicRule,
  rules: &mut Vec<Rule<L>>,
  env: &DeserializeEnv<L>,
) -> Result<(), RuleSerializeError> {
  use Rule as R;
  if let Some(pattern) = atomic.pattern {
    rules.push(match pattern {
      PatternStyle::Str(pat) => R::Pattern(Pattern::try_new(&pat, env.lang.clone())?),
      PatternStyle::Contextual { context, selector } => {
        R::Pattern(Pattern::contextual(&context, &selector, env.lang.clone())?)
      }
    });
  }
  if let Some(kind) = atomic.kind {
    rules.push(R::Kind(KindMatcher::try_new(&kind, env.lang.clone())?));
  }
  if let Some(regex) = atomic.regex {
    rules.push(R::Regex(RegexMatcher::try_new(&regex)?));
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use PatternStyle::*;

  #[test]
  fn test_pattern() {
    let src = r"
pattern: Test
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(rule.pattern.is_present());
    let src = r"
pattern:
  context: class $C { set $B() {} }
  selector: method_definition
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(matches!(rule.pattern, Maybe::Present(Contextual { .. }),));
  }

  #[test]
  fn test_augmentation() {
    let src = r"
pattern: class A {}
inside:
  pattern: function() {}
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(rule.inside.is_present());
    assert!(rule.pattern.is_present());
  }

  #[test]
  fn test_multi_augmentation() {
    let src = r"
pattern: class A {}
inside:
  pattern: function() {}
has:
  pattern: Some()
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(rule.inside.is_present());
    assert!(rule.has.is_present());
    assert!(rule.follows.is_absent());
    assert!(rule.precedes.is_absent());
    assert!(rule.pattern.is_present());
  }

  #[test]
  fn test_maybe_not() {
    let src = "not: 123";
    let ret: Result<SerializableRule, _> = from_str(src);
    assert!(ret.is_err());
    let src = "not:";
    let ret: Result<SerializableRule, _> = from_str(src);
    assert!(ret.is_err());
  }

  #[test]
  fn test_nested_augmentation() {
    let src = r"
pattern: class A {}
inside:
  pattern: function() {}
  inside:
    pattern:
      context: Some()
      selector: ss
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(rule.inside.is_present());
    let inside = rule.inside.unwrap();
    assert!(inside.rule.pattern.is_present());
    assert!(inside.rule.inside.unwrap().rule.pattern.is_present());
  }

  #[test]
  fn test_precedes_follows() {
    let src = r"
pattern: class A {}
precedes:
  pattern: function() {}
follows:
  pattern:
    context: Some()
    selector: ss
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(rule.precedes.is_present());
    assert!(rule.follows.is_present());
    let follows = rule.follows.unwrap();
    assert!(follows.rule.pattern.is_present());
    assert!(follows.rule.pattern.is_present());
  }
}
