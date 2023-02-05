use serde::{Deserialize, Serialize};

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
  pub pattern: Option<PatternStyle>,
  pub kind: Option<String>,
  pub regex: Option<String>,
  // relational
  pub inside: Option<Box<Relation>>,
  pub has: Option<Box<Relation>>,
  pub precedes: Option<Box<Relation>>,
  pub follows: Option<Box<Relation>>,
  // composite
  pub all: Option<Vec<SerializableRule>>,
  pub any: Option<Vec<SerializableRule>>,
  pub not: Option<Box<SerializableRule>>,
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
        pattern: self.pattern,
        kind: self.kind,
        regex: self.regex,
      },
      relational: RelationalRule {
        inside: self.inside,
        has: self.has,
        precedes: self.precedes,
        follows: self.follows,
      },
      composite: CompositeRule {
        all: self.all,
        any: self.any,
        not: self.not,
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
#[serde(rename_all = "camelCase")]
pub enum SerializableStopBy {
  Neighbor,
  #[default]
  End,
  Rule(SerializableRule),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
  #[serde(flatten)]
  pub rule: SerializableRule,
  #[serde(default)]
  pub stop_by: SerializableStopBy,
  pub field: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CompositeRule {
  pub all: Option<Vec<SerializableRule>>,
  pub any: Option<Vec<SerializableRule>>,
  pub not: Option<Box<SerializableRule>>,
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
    assert!(rule.pattern.is_some());
    let src = r"
pattern:
  context: class $C { set $B() {} }
  selector: method_definition
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(matches!(rule.pattern, Some(Contextual { .. }),));
  }

  #[test]
  fn test_relational() {
    let src = r"
inside:
  pattern: class A {}
  stopBy: neighbor
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    let stop_by = rule.inside.unwrap().stop_by;
    assert!(matches!(stop_by, SerializableStopBy::Neighbor));
  }

  #[test]
  fn test_augmentation() {
    let src = r"
pattern: class A {}
inside:
  pattern: function() {}
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(rule.inside.is_some());
    assert!(rule.pattern.is_some());
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
    assert!(rule.inside.is_some());
    assert!(rule.has.is_some());
    assert!(rule.follows.is_none());
    assert!(rule.precedes.is_none());
    assert!(rule.pattern.is_some());
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
    assert!(rule.inside.is_some());
    let inside = rule.inside.unwrap();
    assert!(inside.rule.pattern.is_some());
    assert!(inside.rule.inside.unwrap().rule.pattern.is_some());
  }
}
