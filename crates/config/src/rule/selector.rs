#![allow(clippy::doc_lazy_continuation)]
/// a css selector parser for tree-sitter kind
///
/// Example selector
/// * `call_expression > identifier`
/// is equvalent to
/// ```yaml
/// kind: identifier
/// inside:
///   kind: call_expression
/// ```
/// * `call_expression identifier`
/// is equvalent to
/// ```yaml
/// kind: identifier
/// inside:
///   kind: call_expression
///   stopBy: end
/// ```
/** Grammar for selector

<selector-list> = <complex-selector>#

<complex-selector> = <compound-selector> [ <combinator> <compound-selector> ]*

<compound-selector> = [ <type-selector>? <subclass-selector>* ]!

<combinator> = '>' | '+' | '~' | ' '

<type-selector> = <ident-token>

<subclass-selector> = <class-selector> | <pseudo-class-selector>

<class-selector> = '.' <ident-token>

<pseudo-class-selector> = ':' <ident-token> [ '(' <selector-list> ')' ]?
*/
use super::{
  relational_rule::{Follows, Inside},
  Rule,
};
use ast_grep_core::{
  matcher::{KindMatcher, KindMatcherError},
  ops, Language,
};
use thiserror::Error;

// Inspired by CSS Selector, see
// https://www.w3.org/TR/selectors-4/#grammar
/// Token types for the lexer
#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
  Identifier(&'a str),
  /// + ~ > or space ` `
  Combinator(char),
  /// .
  ClassDot,
  /// :
  PseudoColon,
  /// (
  LeftParen,
  /// )
  RightParen,
  /// ,
  Comma,
}

pub fn parse_selector<L: Language>(source: &str, lang: L) -> Result<Rule, SelectorError> {
  let mut input = Input::new(source, lang);
  let ret = try_parse_selector(&mut input)?;
  if !input.is_empty() {
    return Err(SelectorError::UnexpectedToken);
  }
  Ok(ret)
}

/// <selector-list> = <complex-selector>#
fn try_parse_selector<'a, L: Language>(input: &mut Input<'a, L>) -> Result<Rule, SelectorError> {
  let mut rules = vec![];
  while !input.is_empty() {
    let complex_selector = parse_complex_selector(input)?;
    rules.push(complex_selector);
    if let Some(Token::Comma) = input.peek()? {
      input.next()?; // consume the comma
    } else if !input.is_empty() {
      break;
    }
  }
  Ok(Rule::Any(ops::Any::new(rules)))
}

/// <complex-selector> = <compound-selector> [ <combinator> <compound-selector> ]*
fn parse_complex_selector<'a, L: Language>(
  input: &mut Input<'a, L>,
) -> Result<Rule, SelectorError> {
  let mut rule = parse_compound_selector(input)?;
  loop {
    let Some(combinator) = try_parse_combinator(input)? else {
      break; // no more combinators
    };
    let next_rule = parse_compound_selector(input)?;
    match combinator {
      '>' => {
        rule = Rule::All(ops::All::new([
          next_rule,
          Rule::Inside(Box::new(Inside::rule(rule))),
        ]));
      }
      '+' => {
        rule = Rule::All(ops::All::new([
          next_rule,
          Rule::Follows(Box::new(Follows::rule(rule))),
        ]));
      }
      '~' => {
        rule = Rule::All(ops::All::new([
          next_rule,
          Rule::Follows(Box::new(Follows::rule_descent(rule))),
        ]));
      }
      ' ' => {
        // space combinator means any descendant
        rule = Rule::All(ops::All::new([
          next_rule,
          Rule::Inside(Box::new(Inside::rule_descent(rule))),
        ]));
      }
      _ => {
        return Err(SelectorError::IllegalCharacter(combinator));
      }
    }
  }
  Ok(rule)
}

/// <combinator> = '>' | '+' | '~' | ' '
fn try_parse_combinator<'a, L: Language>(
  input: &mut Input<'a, L>,
) -> Result<Option<char>, SelectorError> {
  let Some(Token::Combinator(c)) = input.peek()? else {
    return Ok(None);
  };
  let c = *c;
  input.next()?; // consume the combinator
  Ok(Some(c))
}

/// <compound-selector> = [ <type-selector>? <subclass-selector>* ]!
fn parse_compound_selector<'a, L: Language>(
  input: &mut Input<'a, L>,
) -> Result<Rule, SelectorError> {
  let mut rules = vec![];
  if let Some(rule) = try_parse_type_selector(input)? {
    rules.push(rule);
  }
  while let Some(subclass_rule) = try_parse_subclass_selector(input)? {
    rules.push(subclass_rule);
  }
  if rules.is_empty() {
    return Err(SelectorError::MissingSelector);
  }
  Ok(Rule::All(ops::All::new(rules)))
}

fn try_parse_type_selector<'a, L: Language>(
  input: &mut Input<'a, L>,
) -> Result<Option<Rule>, SelectorError> {
  let Some(Token::Identifier(ident)) = input.peek()? else {
    return Ok(None);
  };
  let ident = *ident;
  let lang = input.language.clone();
  input.next()?;
  let matcher = KindMatcher::try_new(ident, lang)?;
  Ok(Some(Rule::Kind(matcher)))
}

/// <subclass-selector> = <class-selector> | <pseudo-class-selector>
fn try_parse_subclass_selector<'a, L: Language>(
  input: &mut Input<'a, L>,
) -> Result<Option<Rule>, SelectorError> {
  if let Some(Token::ClassDot) = input.peek()? {
    return Err(SelectorError::Unsupported("class-selector"));
  } else if let Some(Token::PseudoColon) = input.peek()? {
    return Err(SelectorError::Unsupported("pseudo-class-selector"));
  }
  Ok(None)
}

#[derive(Debug, Error)]
pub enum SelectorError {
  #[error("Illegal character {0} encountered")]
  IllegalCharacter(char),
  #[error("Unexpected token")]
  UnexpectedToken,
  #[error("Missing Selector")]
  MissingSelector,
  #[error("Invalid Kind")]
  InvalidKind(#[from] KindMatcherError),
  #[error("{0} is not supported yet")]
  Unsupported(&'static str),
}

struct Input<'a, L: Language> {
  source: &'a str,
  lookahead: Option<Token<'a>>,
  language: L,
}

impl<'a, L: Language> Input<'a, L> {
  fn new(source: &'a str, language: L) -> Self {
    Self {
      source: source.trim(),
      lookahead: None,
      language,
    }
  }

  fn is_empty(&self) -> bool {
    self.source.is_empty() && self.lookahead.is_none()
  }

  fn consume_whitespace(&mut self) {
    self.source = self.source.trim_start();
  }

  fn do_next(&mut self) -> Result<Option<Token<'a>>, SelectorError> {
    if self.source.is_empty() {
      return Ok(None);
    }
    let (next_token, step, need_trim) = match self.source.as_bytes()[0] as char {
      ' ' => {
        let len = self
          .source
          .find(|c: char| !c.is_whitespace())
          .unwrap_or(self.source.len());
        if self.source.len() > len && matches!(self.source.as_bytes()[len] as char, '+' | '~' | '>')
        {
          self.consume_whitespace();
          return self.do_next(); // skip whitespace
        }
        (Token::Combinator(' '), len, true)
      }
      c @ ('+' | '~' | '>') => (Token::Combinator(c), 1, true),
      '.' => (Token::ClassDot, 1, false),
      ':' => (Token::PseudoColon, 1, false),
      '(' => (Token::LeftParen, 1, true),
      ')' => (Token::RightParen, 1, false),
      ',' => (Token::Comma, 1, true),
      'a'..='z' | 'A'..='Z' | '_' | '-' => {
        let len = self
          .source
          .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '-'))
          .unwrap_or(self.source.len());
        let ident = &self.source[..len];
        (Token::Identifier(ident), len, false)
      }
      c => {
        return Err(SelectorError::IllegalCharacter(c));
      }
    };
    self.source = &self.source[step..];
    if need_trim {
      self.consume_whitespace();
    }
    Ok(Some(next_token))
  }

  fn next(&mut self) -> Result<Option<Token<'a>>, SelectorError> {
    if let Some(token) = self.lookahead.take() {
      Ok(Some(token))
    } else {
      self.do_next()
    }
  }

  fn peek(&mut self) -> Result<&Option<Token<'a>>, SelectorError> {
    if self.lookahead.is_some() {
      return Ok(&self.lookahead);
    }
    let next_token = self.do_next()?;
    self.lookahead = next_token;
    Ok(&self.lookahead)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript as TS;
  use ast_grep_core::tree_sitter::LanguageExt;

  fn input_to_tokens(input: &str) -> Result<Vec<Token<'_>>, SelectorError> {
    let mut input = Input::new(input, TS::Tsx);
    let mut tokens = Vec::new();
    while let Some(token) = input.next()? {
      tokens.push(token);
    }
    Ok(tokens)
  }

  #[test]
  fn test_valid_tokens() -> Result<(), SelectorError> {
    let tokens = input_to_tokens("call_expression + statement > .body :has, identifier")?;
    let expected = vec![
      Token::Identifier("call_expression"),
      Token::Combinator('+'),
      Token::Identifier("statement"),
      Token::Combinator('>'),
      Token::ClassDot,
      Token::Identifier("body"),
      Token::Combinator(' '),
      Token::PseudoColon,
      Token::Identifier("has"),
      Token::Comma,
      Token::Identifier("identifier"),
    ];
    assert_eq!(tokens, expected);
    // Test with extra whitespace
    let tokens =
      input_to_tokens("  call_expression   +   statement  >   .body    :has,    identifier  ")?;
    assert_eq!(tokens, expected);
    Ok(())
  }

  #[test]
  fn test_illegal_character() {
    let mut input = Input::new("call_expression $ statement", TS::Tsx);

    assert_eq!(
      input.next().unwrap(),
      Some(Token::Identifier("call_expression"))
    );
    assert_eq!(input.next().unwrap(), Some(Token::Combinator(' ')));
    assert!(matches!(
      input.next(),
      Err(SelectorError::IllegalCharacter('$'))
    ));
  }

  #[test]
  fn test_edge_cases() -> Result<(), SelectorError> {
    // Empty string
    let mut input = Input::new("", TS::Tsx);
    assert_eq!(input.next()?, None);

    // Leading and trailing whitespaces
    let mut input = Input::new("   call_expression   ", TS::Tsx);
    assert_eq!(input.next()?, Some(Token::Identifier("call_expression")));
    assert_eq!(input.next()?, None);

    // Mixed valid and invalid characters
    let mut input = Input::new("call_expression$statement", TS::Tsx);
    assert_eq!(input.next()?, Some(Token::Identifier("call_expression")));
    assert!(matches!(
      input.next(),
      Err(SelectorError::IllegalCharacter('$'))
    ));

    // Long sequence of identifiers
    let mut input = Input::new("thisisaverylongidentifier", TS::Tsx);
    assert_eq!(
      input.next()?,
      Some(Token::Identifier("thisisaverylongidentifier"))
    );
    assert_eq!(input.next()?, None);
    Ok(())
  }

  #[test]
  fn test_parse_selector() -> Result<(), SelectorError> {
    let selector = "call_expression > identifier";
    let rule = parse_selector(selector, TS::Tsx)?;
    let root = TS::Tsx.ast_grep("test(123)");
    let ident = root.root().find(&rule).expect("Should find identifier");
    assert_eq!(ident.kind(), "identifier");
    assert_eq!(ident.text(), "test");
    let rule = parse_selector("call_expression > number", TS::Tsx)?;
    assert!(root.root().find(&rule).is_none());
    let rule = parse_selector("call_expression number", TS::Tsx)?;
    let number = root.root().find(&rule).expect("Should find number");
    assert_eq!(number.text(), "123");
    Ok(())
  }
}
