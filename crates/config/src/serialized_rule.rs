use serde::de::{Deserializer, Error};
use serde::{Deserialize, Serialize};
use serde_yaml::with::singleton_map_recursive::deserialize as deserialize_untagged;
use serde_yaml::{Mapping, Value};

/// We have three kinds of rules in ast-grep.
/// * Atomic: the most basic rule to match AST. We have two variants: Pattern and Kind.
/// * Relational: filter matched target according to their position relative to other nodes.
/// * Composite: use logic operation all/any/not to compose the above rules to larger rules.
#[derive(Serialize, Clone)]
#[serde(untagged, rename_all = "camelCase")]
pub enum SerializableRule {
  Composite(CompositeRule),
  Relational(RelationalRule),
  Atomic {
    #[serde(flatten)]
    rule: AtomicRule,
    #[serde(flatten)]
    augmentation: Augmentation,
  },
}

fn deserialize_rule_variant<'de, D, T>(mapping: Mapping) -> Result<T, D::Error>
where
  D: Deserializer<'de>,
  T: Deserialize<'de>,
{
  match deserialize_untagged(Value::Mapping(mapping)) {
    Ok(result) => Ok(result),
    Err(err) => Err(Error::custom(err)),
  }
}

// SerializableRule can be implmented by derive and untagged enum.
// But for better error message, manual deserialization is needed. #200
// https://serde.rs/deserialize-struct.html
impl<'de> Deserialize<'de> for SerializableRule {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value: Mapping = Deserialize::deserialize(deserializer)?;
    if CompositeRule::should_deserialize(&value) {
      let composite = deserialize_rule_variant::<D, _>(value)?;
      return Ok(SerializableRule::Composite(composite));
    }
    if !AtomicRule::should_deserialize(&value) {
      let relation = deserialize_rule_variant::<D, _>(value)?;
      return Ok(SerializableRule::Relational(relation));
    }
    let mut iter = value.into_iter();
    let atomic = iter.next().into_iter().collect();
    let rule = deserialize_rule_variant::<D, _>(atomic)?;
    let augmentation = deserialize_rule_variant::<D, _>(iter.collect())?;
    Ok(SerializableRule::Atomic { rule, augmentation })
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AtomicRule {
  Pattern(PatternStyle),
  Kind(String),
  Regex(String),
}

// NOTE: adding atomic variant should change this
impl AtomicRule {
  fn should_deserialize(mapping: &Mapping) -> bool {
    mapping.contains_key("pattern") || mapping.contains_key("kind") || mapping.contains_key("regex")
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PatternStyle {
  Str(String),
  Contextual { context: String, selector: String },
}

/// Embed extra conditions into atomic rules to simplify RuleConfig.
/// e.g. a Pattern rule can be augmented with `inside` rule instead of `all` rule.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Augmentation {
  pub inside: Option<Box<Relation>>,
  pub has: Option<Box<Relation>>,
  pub precedes: Option<Box<Relation>>,
  pub follows: Option<Box<Relation>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CompositeRule {
  All(Vec<SerializableRule>),
  Any(Vec<SerializableRule>),
  Not(Box<SerializableRule>),
}

// NOTE: adding composite variant should change this
impl CompositeRule {
  fn should_deserialize(mapping: &Mapping) -> bool {
    mapping.contains_key("all") || mapping.contains_key("any") || mapping.contains_key("not")
  }
}

/// TODO: add doc
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum RelationalRule {
  Inside(Box<Relation>),
  Has(Box<Relation>),
  Precedes(Box<Relation>),
  Follows(Box<Relation>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
  #[serde(flatten)]
  pub rule: SerializableRule,
  #[serde(default)]
  pub until: Option<SerializableRule>,
  #[serde(default)]
  pub immediate: bool,
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use PatternStyle::*;
  use RelationalRule as RR;
  use SerializableRule as S;

  #[test]
  fn test_pattern() {
    let src = r"
pattern: Test
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(matches!(
      rule,
      S::Atomic {
        rule: AtomicRule::Pattern(Str(_)),
        ..
      }
    ));
    let src = r"
pattern:
  context: class $C { set $B() {} }
  selector: method_definition
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(matches!(
      rule,
      S::Atomic {
        rule: AtomicRule::Pattern(Contextual { .. }),
        ..
      }
    ));
  }

  #[test]
  fn test_relational() {
    let src = r"
inside:
  pattern: class A {}
  immediate: true
  until:
    pattern: function() {}
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    match rule {
      S::Relational(RR::Inside(rule)) => assert!(rule.immediate),
      _ => unreachable!(),
    }
  }

  #[test]
  fn test_augmentation() {
    let src = r"
pattern: class A {}
inside:
  pattern: function() {}
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    match rule {
      S::Atomic { rule, augmentation } => {
        assert!(augmentation.inside.is_some());
        assert!(matches!(rule, AtomicRule::Pattern(_)));
      }
      _ => unreachable!(),
    }
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
    match rule {
      S::Atomic { rule, augmentation } => {
        assert!(augmentation.inside.is_some());
        assert!(augmentation.has.is_some());
        assert!(augmentation.follows.is_none());
        assert!(augmentation.precedes.is_none());
        assert!(matches!(rule, AtomicRule::Pattern(_)));
      }
      _ => unreachable!(),
    }
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
    match rule {
      S::Atomic { rule, augmentation } => {
        let inside = augmentation.inside.expect("should parse");
        let Relation { rule: inner, .. } = *inside;
        let nested = match inner {
          S::Atomic { augmentation, .. } => augmentation,
          _ => unreachable!(),
        }
        .inside
        .expect("should parse");
        assert!(matches!(
          nested.rule,
          S::Atomic {
            rule: AtomicRule::Pattern(PatternStyle::Contextual { .. }),
            ..
          }
        ));
        assert!(matches!(rule, AtomicRule::Pattern(_)));
      }
      _ => unreachable!(),
    }
  }
}
