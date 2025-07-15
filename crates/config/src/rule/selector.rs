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

struct Input<'a> {
  source: &'a str,
  offset: usize,
}

impl<'a> Input<'a> {
  fn new(source: &'a str) -> Self {
    Self { source, offset: 0 }
  }
}
