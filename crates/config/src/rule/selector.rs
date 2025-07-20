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

<complex-selector> = <compound-selector> [ <combinator>? <compound-selector> ]*

<compound-selector> = [ <type-selector>? <subclass-selector>* ]!

<combinator> = '>' | '+' | '~'

<type-selector> = <ident-token>

<subclass-selector> = <class-selector> | <pseudo-class-selector>

<class-selector> = '.' <ident-token>

<pseudo-class-selector> = ':' <ident-token> [ '(' <selector-list> ')' ]?
*/
use super::Rule;
use thiserror::Error;

// Inspired by CSS Selector, see
// https://www.w3.org/TR/selectors-4/#grammar
/// Token types for the lexer
#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
  Identifier(&'a str),
  /// + ~ >
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

pub fn parse_selector(source: &str) -> Rule {
  todo!()
}

#[derive(Debug, Error)]
enum SelectorError {
  #[error("Illegal character {0} encountered")]
  IllegalCharacter(char),
}

struct Input<'a> {
  source: &'a str,
}

impl<'a> Input<'a> {
  fn new(source: &'a str) -> Self {
    Self {
      source: source.trim(),
    }
  }

  fn next(&mut self) -> Result<Option<Token<'a>>, SelectorError> {
    if self.source.is_empty() {
      return Ok(None);
    }
    let (next_token, step) = match self.source.as_bytes()[0] as char {
      c @ ('+' | '~' | '>') => (Token::Combinator(c), 1),
      '.' => (Token::ClassDot, 1),
      ':' => (Token::PseudoColon, 1),
      '(' => (Token::LeftParen, 1),
      ')' => (Token::RightParen, 1),
      ',' => (Token::Comma, 1),
      'a'..='z' | 'A'..='Z' | '_' | '-' => {
        let len = self
          .source
          .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '-'))
          .unwrap_or(self.source.len());
        let ident = &self.source[..len];
        (Token::Identifier(ident), len)
      }
      c => {
        return Err(SelectorError::IllegalCharacter(c));
      }
    };
    self.source = self.source[step..].trim_start();
    Ok(Some(next_token))
  }
}

#[cfg(test)]
mod test {
  use super::*;

  fn input_to_tokens(input: &str) -> Result<Vec<Token>, SelectorError> {
    let mut input = Input::new(input);
    let mut tokens = Vec::new();
    while let Some(token) = input.next()? {
      tokens.push(token);
    }
    Ok(tokens)
  }

  #[test]
  fn test_valid_tokens() -> Result<(), SelectorError> {
    let tokens = input_to_tokens("call_expression + statement > .body :has, identifier")?;
    assert_eq!(
      tokens,
      vec![
        Token::Identifier("call_expression"),
        Token::Combinator('+'),
        Token::Identifier("statement"),
        Token::Combinator('>'),
        Token::ClassDot,
        Token::Identifier("body"),
        Token::PseudoColon,
        Token::Identifier("has"),
        Token::Comma,
        Token::Identifier("identifier"),
      ]
    );
    Ok(())
  }

  #[test]
  fn test_illegal_character() {
    let mut input = Input::new("call_expression $ statement");

    assert_eq!(
      input.next().unwrap(),
      Some(Token::Identifier("call_expression"))
    );
    assert!(matches!(
      input.next(),
      Err(SelectorError::IllegalCharacter('$'))
    ));
  }

  #[test]
  fn test_edge_cases() -> Result<(), SelectorError> {
    // Empty string
    let mut input = Input::new("");
    assert_eq!(input.next()?, None);

    // Leading and trailing whitespaces
    let mut input = Input::new("   call_expression   ");
    assert_eq!(input.next()?, Some(Token::Identifier("call_expression")));
    assert_eq!(input.next()?, None);

    // Mixed valid and invalid characters
    let mut input = Input::new("call_expression$statement");
    assert_eq!(input.next()?, Some(Token::Identifier("call_expression")));
    assert!(matches!(
      input.next(),
      Err(SelectorError::IllegalCharacter('$'))
    ));

    // Long sequence of identifiers
    let mut input = Input::new("thisisaverylongidentifier");
    assert_eq!(
      input.next()?,
      Some(Token::Identifier("thisisaverylongidentifier"))
    );
    assert_eq!(input.next()?, None);
    Ok(())
  }
}
