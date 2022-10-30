use super::{try_from_serializable, RelationalRule, Rule, SerializeError};
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Matcher, Node};

use std::marker::PhantomData;

fn until<L: Language>(pattern: &Option<Rule<L>>) -> impl Fn(&Node<L>) -> bool + '_ {
  move |n| {
    if let Some(m) = pattern {
      m.match_node(n.clone()).is_none()
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
  pub fn try_new(relation: RelationalRule, lang: L) -> Result<Inside<L>, SerializeError> {
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
  pub fn try_new(relation: RelationalRule, lang: L) -> Result<Self, SerializeError> {
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
      node
        .dfs()
        .skip(1)
        .take_while(until(&self.until))
        .find_map(|n| self.inner.match_node_with_env(n, env))
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
  pub fn try_new(relation: RelationalRule, lang: L) -> Result<Self, SerializeError> {
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
  pub fn try_new(relation: RelationalRule, lang: L) -> Result<Self, SerializeError> {
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

  #[test]
  fn test_precedes_operator() {
    let precedes = Precedes {
      immediate: false,
      lang: PhantomData,
      until: None,
      inner: Rule::Pattern(Pattern::new("var a = 1", TS::Tsx)),
    };
    test_found(
      &[
        "var b = 2; var a = 1;",
        "var b = 2; var a = 1",
        "var b = 2\n var a = 1",
      ],
      &precedes,
    );
    test_not_found(
      &[
        "var a = 1",
        "var b = 2; var a = 2;",
        "var a = 1; var b = 2;",
      ],
      &precedes,
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
    test_found(
      &[
        "var b = 2; var a = 1;",
        "var b = 2; var a = 1",
        "var b = 2\n var a = 1",
      ],
      &follows,
    );
    test_not_found(
      &["var a = 1", "var b = 2", "var a = 1; var b = 2;"],
      &follows,
    );
  }
}
