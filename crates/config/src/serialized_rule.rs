use serde::{Deserialize, Serialize};

/// We have three kinds of rules in ast-grep.
/// * Atomic: the most basic rule to match AST. We have two variants: Pattern and Kind.
/// * Relational: filter matched target according to their position relative to other nodes.
/// * Composite: use logic operation all/any/not to compose the above rules to larger rules.
#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AtomicRule {
  Pattern(PatternStyle),
  Kind(String),
}

/// Fields for extra conditions on atomic rules to simplify RuleConfig.
/// e.g. a Pattern rule can be augmented with `inside` rule.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Augmentation {
  inside: Option<Box<Relation>>,
  has: Option<Box<Relation>>,
  precedes: Option<Box<Relation>>,
  follows: Option<Box<Relation>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CompositeRule {
  All(Vec<SerializableRule>),
  Any(Vec<SerializableRule>),
  Not(Box<SerializableRule>),
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PatternStyle {
  Str(String),
  Contextual { context: String, selector: String },
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
}
