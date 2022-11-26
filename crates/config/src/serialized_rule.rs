use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged, rename_all = "camelCase")]
pub enum SerializableRule {
  Composite(CompositeRule),
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
  inside: Option<Box<RelationalRule>>,
  has: Option<Box<RelationalRule>>,
  precedes: Option<Box<RelationalRule>>,
  follows: Option<Box<RelationalRule>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CompositeRule {
  All(Vec<SerializableRule>),
  Any(Vec<SerializableRule>),
  Not(Box<SerializableRule>),
  Inside(Box<RelationalRule>),
  Has(Box<RelationalRule>),
  Precedes(Box<RelationalRule>),
  Follows(Box<RelationalRule>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelationalRule {
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
  use CompositeRule as C;
  use PatternStyle::*;
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
      S::Composite(C::Inside(rule)) => assert!(rule.immediate),
      _ => unreachable!(),
    }
  }
}
