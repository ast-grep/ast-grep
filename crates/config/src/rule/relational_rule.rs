use super::deserialize_env::DeserializeEnv;
use super::stop_by::{SerializableStopBy, StopBy};
use crate::rule::{Rule, RuleSerializeError, SerializableRule};
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Matcher, Node};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
  #[serde(flatten)]
  pub rule: SerializableRule,
  #[serde(default)]
  pub stop_by: SerializableStopBy,
  pub field: Option<String>,
}

fn field_name_to_id<L: Language>(
  field: Option<String>,
  env: &DeserializeEnv<L>,
) -> Result<Option<u16>, RuleSerializeError> {
  let Some(field) = field else {
    return Ok(None);
  };
  let ts_lang = env.lang.get_ts_language();
  match ts_lang.field_id_for_name(&field) {
    Some(id) => Ok(Some(id)),
    None => Err(RuleSerializeError::InvalidField(field)),
  }
}

pub struct Inside<L: Language> {
  outer: Rule<L>,
  field: Option<u16>,
  stop_by: StopBy<L>,
}
impl<L: Language> Inside<L> {
  pub fn try_new(relation: Relation, env: &DeserializeEnv<L>) -> Result<Self, RuleSerializeError> {
    Ok(Self {
      stop_by: StopBy::try_from(relation.stop_by, env)?,
      field: field_name_to_id(relation.field, env)?,
      outer: env.deserialize_rule(relation.rule)?, // TODO
    })
  }

  pub fn defined_vars(&self) -> HashSet<&str> {
    self
      .outer
      .defined_vars()
      .union(&self.stop_by.defined_vars())
      .copied()
      .collect()
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    self.outer.verify_util()?;
    self.stop_by.verify_util()
  }
}

impl<L: Language> Matcher<L> for Inside<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let parent = || node.parent();
    let ancestors = || node.ancestors();
    if let Some(field) = self.field {
      let mut last_id = node.node_id();
      let finder = move |nd: Node<'tree, D>| {
        let expect_id = last_id;
        last_id = nd.node_id();
        let n = nd.child_by_field_id(field)?;
        if n.node_id() != expect_id {
          None
        } else {
          self.outer.match_node_with_env(nd, env)
        }
      };
      self.stop_by.find(parent, ancestors, finder)
    } else {
      let finder = |n| self.outer.match_node_with_env(n, env);
      self.stop_by.find(parent, ancestors, finder)
    }
  }
}

pub struct Has<L: Language> {
  inner: Rule<L>,
  stop_by: StopBy<L>,
  field: Option<u16>,
}
impl<L: Language> Has<L> {
  pub fn try_new(relation: Relation, env: &DeserializeEnv<L>) -> Result<Self, RuleSerializeError> {
    Ok(Self {
      stop_by: StopBy::try_from(relation.stop_by, env)?,
      inner: env.deserialize_rule(relation.rule)?,
      field: field_name_to_id(relation.field, env)?,
    })
  }

  pub fn defined_vars(&self) -> HashSet<&str> {
    self
      .inner
      .defined_vars()
      .union(&self.stop_by.defined_vars())
      .copied()
      .collect()
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    self.inner.verify_util()?;
    self.stop_by.verify_util()
  }
}

impl<L: Language> Matcher<L> for Has<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    if let Some(field) = self.field {
      let nd = node.child_by_field_id(field)?;
      return match &self.stop_by {
        StopBy::Neighbor => self.inner.match_node_with_env(nd, env),
        StopBy::End => nd
          .dfs()
          .find_map(|n| self.inner.match_node_with_env(n, env)),
        StopBy::Rule(matcher) => {
          // TODO: use Pre traversal to reduce stack allocation
          self.inner.match_node_with_env(nd.clone(), env).or_else(|| {
            if nd.matches(matcher) {
              None
            } else {
              nd.children()
                .find_map(|n| self.inner.match_node_with_env(n, env))
            }
          })
        }
      };
    }
    match &self.stop_by {
      StopBy::Neighbor => node
        .children()
        .find_map(|n| self.inner.match_node_with_env(n, env)),
      StopBy::End => node
        .dfs()
        .skip(1)
        .find_map(|n| self.inner.match_node_with_env(n, env)),
      StopBy::Rule(matcher) => {
        // TODO: use Pre traversal to reduce stack allocation
        node.children().find_map(|n| {
          self.inner.match_node_with_env(n.clone(), env).or_else(|| {
            if n.matches(matcher) {
              None
            } else {
              self.match_node_with_env(n, env)
            }
          })
        })
      }
    }
  }
}

pub struct Precedes<L: Language> {
  later: Rule<L>,
  stop_by: StopBy<L>,
}
impl<L: Language> Precedes<L> {
  pub fn try_new(relation: Relation, env: &DeserializeEnv<L>) -> Result<Self, RuleSerializeError> {
    if relation.field.is_some() {
      return Err(RuleSerializeError::FieldNotSupported);
    }
    Ok(Self {
      stop_by: StopBy::try_from(relation.stop_by, env)?,
      later: env.deserialize_rule(relation.rule)?,
    })
  }

  pub fn defined_vars(&self) -> HashSet<&str> {
    self
      .later
      .defined_vars()
      .union(&self.stop_by.defined_vars())
      .copied()
      .collect()
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    self.later.verify_util()?;
    self.stop_by.verify_util()
  }
}
impl<L: Language> Matcher<L> for Precedes<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let next = || node.next();
    let next_all = || node.next_all();
    let finder = |n| self.later.match_node_with_env(n, env);
    self.stop_by.find(next, next_all, finder)
  }
}

pub struct Follows<L: Language> {
  former: Rule<L>,
  stop_by: StopBy<L>,
}
impl<L: Language> Follows<L> {
  pub fn try_new(relation: Relation, env: &DeserializeEnv<L>) -> Result<Self, RuleSerializeError> {
    if relation.field.is_some() {
      return Err(RuleSerializeError::FieldNotSupported);
    }
    Ok(Self {
      stop_by: StopBy::try_from(relation.stop_by, env)?,
      former: env.deserialize_rule(relation.rule)?,
    })
  }
  pub fn defined_vars(&self) -> HashSet<&str> {
    self
      .former
      .defined_vars()
      .union(&self.stop_by.defined_vars())
      .copied()
      .collect()
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    self.former.verify_util()?;
    self.stop_by.verify_util()
  }
}
impl<L: Language> Matcher<L> for Follows<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let prev = || node.prev();
    let prev_all = || node.prev_all();
    let finder = |n| self.former.match_node_with_env(n, env);
    self.stop_by.find(prev, prev_all, finder)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript as TS;
  use ast_grep_core::matcher::KindMatcher;
  use ast_grep_core::ops as o;
  use ast_grep_core::Pattern;

  fn find_rule<M: Matcher<TS>>(src: &str, matcher: M) -> Option<String> {
    let grep = TS::Tsx.ast_grep(src);
    grep.root().find(matcher).map(|s| s.text().to_string())
  }

  fn test_found<M: Matcher<TS>>(found_list: &[&str], matcher: M) {
    for found in found_list {
      assert!(find_rule(found, &matcher).is_some());
    }
  }

  fn test_not_found<M: Matcher<TS>>(not_found_list: &[&str], matcher: M) {
    for found in not_found_list {
      assert!(find_rule(found, &matcher).is_none());
    }
  }

  fn make_rule(target: &str, relation: Rule<TS>) -> impl Matcher<TS> {
    o::All::new(vec![Rule::Pattern(Pattern::new(target, TS::Tsx)), relation])
  }

  #[test]
  fn test_precedes_operator() {
    let precedes = Precedes {
      later: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
      stop_by: StopBy::End,
    };
    let rule = make_rule("var b = 2", Rule::Precedes(Box::new(precedes)));
    test_found(
      &[
        "var b = 2; var a = 1;",
        "var b = 2; alert(b); var a = 1;",
        "var b = 2; var a = 1",
        "var b = 2\n var a = 1",
      ],
      &rule,
    );
    test_not_found(
      &[
        "var a = 1",
        "var b = 2; var a = 2;",
        "var a = 1; var b = 2;",
        "{ var a = 1 }",
        "var b = 2; { var a = 1 }",
      ],
      &rule,
    );
  }

  #[test]
  fn test_precedes_immediate() {
    let precedes = Precedes {
      later: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
      stop_by: StopBy::Neighbor,
    };
    let rule = make_rule("var b = 2", Rule::Precedes(Box::new(precedes)));
    test_found(
      &[
        "var b = 2; var a = 1;",
        "var b = 2; var a = 1",
        "var b = 2\n var a = 1",
        "{ var b = 2; var a = 1 }",
        "function test() { var b = 2; var a = 1 }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "var a = 1",
        "var b = 2; var a = 2;",
        "var a = 1; var b = 2;",
        "var b = 2; alert(b); var a = 1;",
        "{ var b = 2 } var a = 1;",
      ],
      &rule,
    );
  }

  #[test]
  fn test_follows_operator() {
    let follows = Follows {
      former: Rule::Pattern(Pattern::new("var b = 2", TS::Tsx)),
      stop_by: StopBy::End,
    };
    let rule = make_rule("var a = 1", Rule::Follows(Box::new(follows)));
    test_found(
      &[
        "var b = 2; var a = 1;",
        "var b = 2; var a = 1",
        "var b = 2; alert(b); var a = 1",
        "var b = 2\n var a = 1",
        "alert(b); var b = 2; var a = 1",
        "{var b = 2; var a = 1;}", // inside block
      ],
      &rule,
    );
    test_not_found(
      &[
        "var a = 1",
        "var b = 2",
        "var a = 1; var b = 2;",
        "var a = 1; alert(b) ;var b = 2;",
        "var a = 1\n var b = 2;",
        "{var b = 2;} var a = 1;", // inside block
      ],
      &rule,
    );
  }

  #[test]
  fn test_follows_immediate() {
    let follows = Follows {
      former: Rule::Pattern(Pattern::new("var b = 2", TS::Tsx)),
      stop_by: StopBy::Neighbor,
    };
    let rule = make_rule("var a = 1", Rule::Follows(Box::new(follows)));
    test_found(
      &[
        "var b = 2; var a = 1;",
        "var b = 2; var a = 1",
        "var b = 2\n var a = 1",
        "alert(b); var b = 2; var a = 1",
        "{var b = 2; var a = 1;}", // inside block
      ],
      &rule,
    );
    test_not_found(
      &[
        "var a = 1",
        "var b = 2",
        "var a = 1; var b = 2;",
        "var a = 1; alert(b) ;var b = 2;",
        "var a = 1\n var b = 2;",
        "var b = 2; alert(b); var a = 1", // not immediate
        "{var b = 2;} var a = 1;",        // inside block
      ],
      &rule,
    );
  }

  #[test]
  fn test_has_rule() {
    let has = Has {
      stop_by: StopBy::End,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
      field: None,
    };
    let rule = make_rule("function test() { $$$ }", Rule::Has(Box::new(has)));
    test_found(
      &[
        "function test() { var a = 1 }",
        "function test() { var a = 1; var b = 2 }",
        "function test() { function nested() { var a = 1 } }",
        "function test() { if (nested) { var a = 1 } }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "var test = function () { var a = 2 }",
        "function test() { var a = 2 }",
        "function test() { let a = 1; var b = 2 }",
        "if (test) {  { var a = 1 } }",
      ],
      &rule,
    );
  }

  #[test]
  fn test_has_until_should_not_abort_prematurely() {
    let has = Has {
      stop_by: StopBy::Rule(Rule::Kind(KindMatcher::new(
        "function_declaration",
        TS::Tsx,
      ))),
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
      field: None,
    };
    let rule = make_rule("function test() { $$$ }", Rule::Has(Box::new(has)));
    test_found(
      &[
        "function test() { var a = 1}",
        "function test() { function inner() { var a = 1 }; var a = 1}",
      ],
      &rule,
    );
    test_not_found(
      &[
        "function test() { var a = 2}",
        "function test() { function inner() { var a = 1 }}",
      ],
      &rule,
    );
  }

  #[test]
  fn test_has_until_should_be_inclusive() {
    let has = Has {
      stop_by: StopBy::Rule(Rule::Kind(KindMatcher::new(
        "function_declaration",
        TS::Tsx,
      ))),
      inner: Rule::Pattern(Pattern::new("function inner() {$$$}", TS::Tsx)),
      field: None,
    };
    let rule = make_rule("function test() { $$$ }", Rule::Has(Box::new(has)));
    test_found(
      &[
        "function test() { function inner() { var a = 1 };}",
        "function test() { var a = 1; function inner() { var a = 1 };}",
        "function test() { if (false) { function inner() { var a = 1 };} }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "function test() { var a = 2}",
        "function test() { function bbb() { function inner() { var a = 1 } }}",
      ],
      &rule,
    );
  }

  #[test]
  fn test_has_immediate() {
    let has = Has {
      stop_by: StopBy::Neighbor,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
      field: None,
    };
    let rule = o::All::new(vec![
      Rule::Pattern(Pattern::new("{ $$$ }", TS::Tsx)),
      Rule::Inside(Box::new(Inside {
        outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
        stop_by: StopBy::Neighbor,
        field: None,
      })),
      Rule::Has(Box::new(has)),
    ]);
    test_found(
      &[
        "function test() { var a = 1 }",
        "function test() { var a = 1; var b = 2 }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "var test = function () { var a = 2 }",
        "function test() { var a = 2 }",
        "function test() { let a = 1; var b = 2 }",
        "if (test) {  { var a = 1 } }",
        // nested
        "function test() { if (nested) { var a = 1 } }",
        "function test() { function nested() { var a = 1 } }",
      ],
      &rule,
    );
  }

  #[test]
  fn test_inside_rule() {
    let inside = Inside {
      stop_by: StopBy::End,
      outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
      field: None,
    };
    let rule = make_rule("var a = 1", Rule::Inside(Box::new(inside)));
    test_found(
      &[
        "function test() { var a = 1 }",
        "function test() { var a = 1; var b = 2 }",
        "function test() { function nested() { var a = 1 } }",
        "function test() { if (nested) { var a = 1 } }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "var test = function () { var a = 2 }",
        "function test() { var a = 2 }",
        "function test() { let a = 1; var b = 2 }",
        "if (test) {  { var a = 1 } }",
      ],
      &rule,
    );
  }

  #[test]
  fn test_inside_inclusive() {
    let inside = Inside {
      stop_by: StopBy::Rule(Rule::Kind(KindMatcher::new(
        "function_declaration",
        TS::Tsx,
      ))),
      outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
      field: None,
    };
    let rule = make_rule("var a = 1", Rule::Inside(Box::new(inside)));
    test_found(
      &[
        "function test() { var a = 1 }",
        "function test() { var a = 1; var b = 2 }",
        "function test() { if (nested) { var a = 1 } }",
        "function test() { var b = function(nested) { var a = 1 } }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "function test() { function nested() { var a = 1 } }",
        "var test = function () { var a = 2 }",
        "function test() { var a = 2 }",
        "function test() { let a = 1; var b = 2 }",
      ],
      &rule,
    );
  }

  #[test]
  fn test_inside_immediate() {
    let inside = Inside {
      stop_by: StopBy::Neighbor,
      outer: Rule::All(o::All::new(vec![
        Rule::Pattern(Pattern::new("{ $$$ }", TS::Tsx)),
        Rule::Inside(Box::new(Inside {
          outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
          stop_by: StopBy::Neighbor,
          field: None,
        })),
      ])),
      field: None,
    };
    let rule = make_rule("var a = 1", Rule::Inside(Box::new(inside)));
    test_found(
      &[
        "function test() { var a = 1 }",
        "function test() { var a = 1; var b = 2 }",
      ],
      &rule,
    );
    test_not_found(
      &[
        "var test = function () { var a = 2 }",
        "function test() { var a = 2 }",
        "function test() { let a = 1; var b = 2 }",
        "if (test) {  { var a = 1 } }",
        // nested
        "function test() { function nested() { var a = 1 } }",
        "function test() { if (nested) { var a = 1 } }",
      ],
      &rule,
    );
  }

  #[test]
  fn test_inside_field() {
    let inside = Inside {
      stop_by: StopBy::End,
      outer: Rule::Kind(KindMatcher::new("for_statement", TS::Tsx)),
      field: TS::Tsx.get_ts_language().field_id_for_name("condition"),
    };
    let rule = make_rule("a = 1", Rule::Inside(Box::new(inside)));
    test_found(&["for (;a = 1;) {}"], &rule);
    test_not_found(&["for (;; a = 1) {}"], &rule);
  }

  #[test]
  fn test_has_field() {
    let has = Has {
      stop_by: StopBy::End,
      inner: Rule::Pattern(Pattern::new("a = 1", TS::Tsx)),
      field: TS::Tsx.get_ts_language().field_id_for_name("condition"),
    };
    let rule = o::All::new(vec![
      Rule::Kind(KindMatcher::new("for_statement", TS::Tsx)),
      Rule::Has(Box::new(has)),
    ]);
    test_found(&["for (;a = 1;) {}"], &rule);
    test_not_found(&["for (;; a = 1) {}", "for (;;) { a = 1}"], &rule);
  }

  #[test]
  fn test_invalid_field() {
    let env = DeserializeEnv::new(TS::Tsx);
    let relation = Relation {
      rule: crate::from_str("pattern: test").unwrap(),
      stop_by: SerializableStopBy::End,
      field: Some("invalid_field".to_string()),
    };
    let inside = Inside::try_new(relation, &env);
    assert!(inside.is_err());
    match inside {
      Err(RuleSerializeError::InvalidField(_)) => {}
      _ => panic!("expected InvalidField error"),
    }
  }

  #[test]
  fn test_defined_vars() {
    let precedes = Precedes {
      later: Rule::Pattern(Pattern::new("var a = $A", TS::Tsx)),
      stop_by: StopBy::Rule(Rule::Pattern(Pattern::new("var b = $B", TS::Tsx))),
    };
    assert_eq!(precedes.defined_vars(), ["A", "B"].into_iter().collect());
    let follows = Follows {
      former: Rule::Pattern(Pattern::new("var a = 123", TS::Tsx)),
      stop_by: StopBy::Rule(Rule::Pattern(Pattern::new("var b = $B", TS::Tsx))),
    };
    assert_eq!(follows.defined_vars(), ["B"].into_iter().collect());
    let inside = Inside {
      stop_by: StopBy::Rule(Rule::Pattern(Pattern::new("var $C", TS::Tsx))),
      outer: Rule::Pattern(Pattern::new("var a = $A", TS::Tsx)),
      field: TS::Tsx.get_ts_language().field_id_for_name("condition"),
    };
    assert_eq!(inside.defined_vars(), ["A", "C"].into_iter().collect());
    let has = Has {
      stop_by: StopBy::Rule(Rule::Kind(KindMatcher::new("for_statement", TS::Tsx))),
      inner: Rule::Pattern(Pattern::new("var a = $A", TS::Tsx)),
      field: TS::Tsx.get_ts_language().field_id_for_name("condition"),
    };
    assert_eq!(has.defined_vars(), ["A"].into_iter().collect());
  }
}
