use super::{DeserializeEnv, Rule, RuleSerializeError, SerializableRule};

use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Matcher, Node};

use std::borrow::Cow;
use std::collections::HashSet;

use bit_set::BitSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NthChildError {
  #[error("Illegal character {0} encountered")]
  IllegalCharacter(char),
  #[error("Invalid syntax")]
  InvalidSyntax,
  #[error("Invalid ofRule")]
  InvalidRule(#[from] Box<RuleSerializeError>),
}

/// A string or number describing the indices of matching nodes in a list of siblings.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum NthChildSimple {
  /// A number indicating the precise element index
  Numeric(usize),
  /// Functional notation like CSS's An + B
  Functional(String),
}

enum ParseState {
  Initial,
  N,
  Sign(bool), // bool flag: has met n before?
  Num(bool),  // bool flag: has met n before
}

fn parse_an_b(input: &str) -> Result<FunctionalPosition, NthChildError> {
  use ParseState::*;
  let mut step_size = 0;
  let mut sign = 1;
  let mut num = 0;
  let mut state = Initial;
  for c in input.chars() {
    // ignore all white spaces
    if c.is_whitespace() {
      continue;
    }
    match state {
      Initial => match c {
        '+' | '-' => {
          state = Sign(false);
          sign = if c == '+' { 1 } else { -1 };
        }
        '0'..='9' => {
          state = Num(false);
          num = (c as u8 - b'0') as i32;
        }
        'n' | 'N' => {
          state = N;
          step_size = sign;
        }
        c => return Err(NthChildError::IllegalCharacter(c)),
      },
      Sign(has_n) => match c {
        '+' | '-' => return Err(NthChildError::InvalidSyntax),
        '0'..='9' => {
          state = Num(has_n);
          num = (c as u8 - b'0') as i32;
        }
        'n' | 'N' => {
          if has_n {
            return Err(NthChildError::InvalidSyntax);
          }
          state = N;
          step_size = sign;
        }
        c => return Err(NthChildError::IllegalCharacter(c)),
      },
      Num(has_n) => match c {
        '+' | '-' => return Err(NthChildError::InvalidSyntax),
        '0'..='9' => {
          num = num * 10 + (c as u8 - b'0') as i32;
        }
        'n' | 'N' => {
          if has_n {
            return Err(NthChildError::InvalidSyntax);
          }
          state = N;
          step_size = sign * num;
          num = 0;
        }
        c => return Err(NthChildError::IllegalCharacter(c)),
      },
      N => match c {
        '+' | '-' => {
          state = Sign(true);
          sign = if c == '+' { 1 } else { -1 };
          num = 0;
        }
        '0'..='9' => return Err(NthChildError::InvalidSyntax),
        'n' | 'N' => return Err(NthChildError::InvalidSyntax),
        c => return Err(NthChildError::IllegalCharacter(c)),
      },
    }
  }
  if matches!(state, Sign(_) | Initial) {
    Err(NthChildError::InvalidSyntax)
  } else {
    Ok(FunctionalPosition {
      step_size,
      offset: num * sign,
    })
  }
}

impl NthChildSimple {
  fn try_parse(&self) -> Result<FunctionalPosition, NthChildError> {
    match self {
      NthChildSimple::Numeric(n) => Ok(FunctionalPosition {
        step_size: 0,
        offset: *n as i32,
      }),
      NthChildSimple::Functional(s) => parse_an_b(s),
    }
  }
}

/// `nthChild` accepts either a number, a string or an object.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged, rename_all = "camelCase")]
pub enum SerializableNthChild {
  /// Simple syntax
  Simple(NthChildSimple),
  /// Object style syntax
  #[serde(rename_all = "camelCase")]
  Complex {
    /// nth-child syntax
    position: NthChildSimple,
    /// select the nth node that matches the rule, like CSS's of syntax
    of_rule: Option<Box<SerializableRule>>,
    /// matches from the end instead like CSS's nth-last-child
    #[serde(default)]
    reverse: bool,
  },
}

/// Corresponds to the CSS syntax An+B
/// See https://developer.mozilla.org/en-US/docs/Web/CSS/:nth-child#functional_notation
struct FunctionalPosition {
  step_size: i32,
  offset: i32,
}

impl FunctionalPosition {
  /// index is 0-based, but output is 1-based
  fn is_matched(&self, index: usize) -> bool {
    let index = (index + 1) as i32; // Convert 0-based index to 1-based
    let FunctionalPosition { step_size, offset } = self;
    if *step_size == 0 {
      index == *offset
    } else {
      let n = index - offset;
      n / step_size >= 0 && n % step_size == 0
    }
  }
}

pub struct NthChild<L: Language> {
  position: FunctionalPosition,
  of_rule: Option<Box<Rule<L>>>,
  reverse: bool,
}

impl<L: Language> NthChild<L> {
  pub fn try_new(
    rule: SerializableNthChild,
    env: &DeserializeEnv<L>,
  ) -> Result<Self, NthChildError> {
    match rule {
      SerializableNthChild::Simple(position) => Ok(NthChild {
        position: position.try_parse()?,
        of_rule: None,
        reverse: false,
      }),
      SerializableNthChild::Complex {
        position,
        of_rule,
        reverse,
      } => Ok(NthChild {
        position: position.try_parse()?,
        of_rule: of_rule
          .map(|r| env.deserialize_rule(*r))
          .transpose()
          .map_err(Box::new)?
          .map(Box::new),
        reverse,
      }),
    }
  }

  fn find_index<'t, D: Doc<Lang = L>>(
    &self,
    node: &Node<'t, D>,
    env: &mut Cow<MetaVarEnv<'t, D>>,
  ) -> Option<usize> {
    let parent = node.parent()?;
    //  only consider named children
    let mut children: Vec<_> = if let Some(rule) = &self.of_rule {
      // if of_rule is present, only consider children that match the rule
      parent
        .children()
        .filter(|n| n.is_named())
        .filter_map(|child| rule.match_node_with_env(child, env))
        .collect()
    } else {
      parent.children().filter(|n| n.is_named()).collect()
    };
    // count the index from the end if reverse is true
    if self.reverse {
      children.reverse()
    }
    children
      .iter()
      .position(|child| child.node_id() == node.node_id())
  }
  pub fn defined_vars(&self) -> HashSet<&str> {
    if let Some(rule) = &self.of_rule {
      rule.defined_vars()
    } else {
      HashSet::new()
    }
  }

  pub fn verify_util(&self) -> Result<(), RuleSerializeError> {
    if let Some(rule) = &self.of_rule {
      rule.verify_util()
    } else {
      Ok(())
    }
  }
}

impl<L: Language> Matcher<L> for NthChild<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    let index = self.find_index(&node, env)?;
    self.position.is_matched(index).then_some(node)
  }
  fn potential_kinds(&self) -> Option<BitSet> {
    let rule = self.of_rule.as_ref()?;
    rule.potential_kinds()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript as TS;
  use ast_grep_core::matcher::RegexMatcher;

  #[test]
  fn test_positional() {
    let position = FunctionalPosition {
      step_size: 0,
      offset: 1,
    };
    assert!(position.is_matched(0));
    assert!(!position.is_matched(1));
    assert!(!position.is_matched(2));
  }

  #[test]
  fn test_positional_an_b() {
    let position = FunctionalPosition {
      step_size: 2,
      offset: -1,
    };
    assert!(position.is_matched(0));
    assert!(!position.is_matched(1));
    assert!(position.is_matched(2));
    assert!(!position.is_matched(3));
    assert!(position.is_matched(4));
  }

  fn find_index(rule: Option<Rule<TS>>, reverse: bool) -> Option<usize> {
    let rule = NthChild {
      position: FunctionalPosition {
        step_size: 2,
        offset: -1,
      },
      of_rule: rule.map(Box::new),
      reverse,
    };
    let mut env = Cow::Owned(MetaVarEnv::new());
    let grep = TS::Tsx.ast_grep("[1,2,3,4]");
    let node = grep.root().find("2").unwrap();
    rule.find_index(&node, &mut env)
  }

  #[test]
  fn test_find_index_simple() {
    let i = find_index(None, false);
    assert_eq!(i, Some(1));
  }

  #[test]
  fn test_find_index_reverse() {
    let i = find_index(None, true);
    assert_eq!(i, Some(2));
  }

  #[test]
  fn test_find_of_rule() {
    let regex = RegexMatcher::try_new(r"2|3").unwrap();
    let i = find_index(Some(Rule::Regex(regex.clone())), false);
    assert_eq!(i, Some(0));
    let i = find_index(Some(Rule::Regex(regex)), true);
    assert_eq!(i, Some(1));
  }

  fn parse(s: &str) -> FunctionalPosition {
    parse_an_b(s).expect("should parse")
  }
  fn test_parse(s: &str, step: i32, offset: i32) {
    let pos = parse(s);
    assert_eq!(pos.step_size, step, "{s}: wrong step");
    assert_eq!(pos.offset, offset, "{s}: wrong offset");
  }

  #[test]
  fn test_parse_selector() {
    // https://www.w3.org/TR/css-syntax-3/#anb-microsyntax
    test_parse("12n + 2", 12, 2);
    test_parse("-12n + 21", -12, 21);
    test_parse("-12n - 21", -12, -21);
    test_parse("2n + 0", 2, 0);
    test_parse("-1n + 6", -1, 6);
    test_parse("-4n + 10", -4, 10);
    test_parse("0n + 5", 0, 5);
    test_parse("2", 0, 2);
    test_parse("-2", 0, -2);
    test_parse("n", 1, 0);
    test_parse("-n", -1, 0);
    test_parse("N", 1, 0);
    test_parse("-N", -1, 0);
    test_parse("123   n", 123, 0);
  }

  fn parse_error(s: &str, name: &str) {
    let Err(err) = parse_an_b(s) else {
      panic!("should parse error: {s}");
    };
    match err {
      NthChildError::InvalidSyntax => assert_eq!(name, "syntax"),
      NthChildError::IllegalCharacter(_) => assert_eq!(name, "character"),
      NthChildError::InvalidRule(_) => assert_eq!(name, "rule"),
    }
  }

  #[test]
  fn test_error() {
    parse_error("3a + b", "character");
    parse_error("3 - n", "syntax");
    parse_error("3 ++ n", "syntax");
    parse_error("n++", "syntax");
    parse_error("3 + 5", "syntax");
    parse_error("3n +", "syntax");
    parse_error("3n + n", "syntax");
    parse_error("n + 3n", "syntax");
    parse_error("+ n + n", "syntax");
    parse_error("+ n - n", "syntax");
    parse_error("nN", "syntax");
    parse_error("+", "syntax");
    parse_error("-", "syntax");
    parse_error("a", "character");
    parse_error("+a", "character");
    parse_error("na", "character");
  }

  fn deser(src: &str) -> Rule<TS> {
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    let env = DeserializeEnv::new(TS::Tsx);
    env.deserialize_rule(rule).expect("should deserialize")
  }

  #[test]
  fn test_serialize() {
    let root = TS::Tsx.ast_grep("[1,2,3,4]");
    let root = root.root();
    let rule = deser(r"nthChild: 3");
    assert_eq!(root.find(rule).expect("should find").text(), "3");
    let rule = deser(r"nthChild: { position: 2n + 2 }");
    assert_eq!(root.find(rule).expect("should find").text(), "2");
    let rule = deser(r"nthChild: { position: 2n + 2, reverse: true }");
    assert_eq!(root.find(rule).expect("should find").text(), "1");
    let rule = deser(r"nthChild: { position: 2n + 2, ofRule: {regex: '2|3'} }");
    assert_eq!(root.find(rule).expect("should find").text(), "3");
  }

  #[test]
  fn test_defined_vars() {
    let rule = deser(r"nthChild: { position: 2, ofRule: {pattern: '$A'} }");
    assert_eq!(rule.defined_vars(), vec!["A"].into_iter().collect());
  }

  #[test]
  fn test_verify_util() {
    let rule = deser(r"nthChild: { position: 2, ofRule: {pattern: '$A'} }");
    assert!(rule.verify_util().is_ok());
  }
}
