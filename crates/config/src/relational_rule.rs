use crate::rule_config::{try_from_serializable, Rule, RuleSerializeError};
use crate::serialized_rule::Relation;
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Matcher, Node};

use std::marker::PhantomData;

fn until<L: Language>(pattern: &Option<Rule<L>>) -> impl Fn(&Node<L>) -> bool + '_ {
  move |n| {
    if let Some(m) = pattern {
      !n.matches(m)
    } else {
      true
    }
  }
}

pub struct Inside<L: Language> {
  outer: Rule<L>,
  until: Option<Rule<L>>,
  immediate: bool,
  lang: PhantomData<L>,
}
impl<L: Language> Inside<L> {
  pub fn try_new(relation: Relation, lang: L) -> Result<Inside<L>, RuleSerializeError> {
    let util_node = if let Some(until) = relation.until {
      Some(try_from_serializable(until, lang.clone())?)
    } else {
      None
    };
    Ok(Self {
      outer: try_from_serializable(relation.rule, lang)?,
      until: util_node,
      immediate: relation.immediate,
      lang: PhantomData,
    })
  }
}

impl<L: Language> Matcher<L> for Inside<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    if self.immediate {
      self.outer.match_node_with_env(node.parent()?, env)
    } else {
      node
        .ancestors()
        .take_while(until(&self.until))
        .find_map(|n| self.outer.match_node_with_env(n, env))
    }
  }
}

pub struct Has<L: Language> {
  inner: Rule<L>,
  until: Option<Rule<L>>,
  immediate: bool,
  lang: PhantomData<L>,
}
impl<L: Language> Has<L> {
  pub fn try_new(relation: Relation, lang: L) -> Result<Self, RuleSerializeError> {
    let util_node = if let Some(until) = relation.until {
      Some(try_from_serializable(until, lang.clone())?)
    } else {
      None
    };
    Ok(Self {
      inner: try_from_serializable(relation.rule, lang)?,
      until: util_node,
      immediate: relation.immediate,
      lang: PhantomData,
    })
  }
}

impl<L: Language> Matcher<L> for Has<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    if self.immediate {
      node
        .children()
        .find_map(|n| self.inner.match_node_with_env(n, env))
    } else {
      // TODO: use Pre traversal to reduce stack allocation
      node.children().filter(until(&self.until)).find_map(|n| {
        self
          .inner
          .match_node_with_env(n.clone(), env)
          .or_else(|| self.match_node_with_env(n, env))
      })
    }
  }
}

pub struct Precedes<L: Language> {
  inner: Rule<L>,
  until: Option<Rule<L>>,
  immediate: bool,
  lang: PhantomData<L>,
}
impl<L: Language> Precedes<L> {
  pub fn try_new(relation: Relation, lang: L) -> Result<Self, RuleSerializeError> {
    let util_node = if let Some(until) = relation.until {
      Some(try_from_serializable(until, lang.clone())?)
    } else {
      None
    };
    Ok(Self {
      inner: try_from_serializable(relation.rule, lang)?,
      until: util_node,
      immediate: relation.immediate,
      lang: PhantomData,
    })
  }
}
impl<L: Language> Matcher<L> for Precedes<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    if self.immediate {
      self.inner.match_node_with_env(node.next()?, env)
    } else {
      node
        .next_all()
        .take_while(until(&self.until))
        .find_map(|n| self.inner.match_node_with_env(n, env))
    }
  }
}

pub struct Follows<L: Language> {
  inner: Rule<L>,
  until: Option<Rule<L>>,
  immediate: bool,
  lang: PhantomData<L>,
}
impl<L: Language> Follows<L> {
  pub fn try_new(relation: Relation, lang: L) -> Result<Self, RuleSerializeError> {
    let util_node = if let Some(until) = relation.until {
      Some(try_from_serializable(until, lang.clone())?)
    } else {
      None
    };
    Ok(Self {
      inner: try_from_serializable(relation.rule, lang)?,
      until: util_node,
      immediate: relation.immediate,
      lang: PhantomData,
    })
  }
}
impl<L: Language> Matcher<L> for Follows<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    if self.immediate {
      self.inner.match_node_with_env(node.prev()?, env)
    } else {
      node
        .prev_all()
        .take_while(until(&self.until))
        .find_map(|n| self.inner.match_node_with_env(n, env))
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript as TS;
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
      immediate: false,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
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
      immediate: true,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
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
      immediate: false,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var b = 2", TS::Tsx)),
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
      immediate: true,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var b = 2", TS::Tsx)),
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
      immediate: false,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
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
  fn test_has_immediate() {
    let has = Has {
      immediate: true,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
    };
    let rule = o::All::new(vec![
      Rule::Pattern(Pattern::new("{ $$$ }", TS::Tsx)),
      Rule::Inside(Box::new(Inside {
        outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
        until: None,
        immediate: true,
        lang: PhantomData,
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
      immediate: false,
      lang: PhantomData,
      until: None,
      outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
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
  fn test_inside_immediate() {
    let inside = Inside {
      immediate: true,
      lang: PhantomData,
      until: None,
      outer: Rule::All(o::All::new(vec![
        Rule::Pattern(Pattern::new("{ $$$ }", TS::Tsx)),
        Rule::Inside(Box::new(Inside {
          outer: Rule::Pattern(Pattern::new("function test() { $$$ }", TS::Tsx)),
          until: None,
          immediate: true,
          lang: PhantomData,
        })),
      ])),
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
}
