use super::deserialize_env::DeserializeEnv;
use super::relational_rule::{Inside, Relation};
use super::Rule;
use crate::maybe::Maybe;
use crate::rule::{RuleSerializeError, SerializableRule};
use ast_grep_core::language::Language;
use ast_grep_core::matcher::KindMatcher;

/// Parse CSS selector-style string into a Rule
pub fn parse_selector<L: Language>(
  selector: &str,
  env: &DeserializeEnv<L>,
) -> Result<Rule, RuleSerializeError> {
  let trimmed = selector.trim();
  if trimmed.is_empty() {
    return Err(RuleSerializeError::MissPositiveMatcher);
  }

  // Parse the selector by splitting on combinators
  let parts = parse_selector_parts(trimmed);
  
  // Convert to nested inside rules
  convert_parts_to_rule(parts, env)
}

#[derive(Debug, Clone)]
enum Combinator {
  Descendant,  // space
  Child,       // >
}

#[derive(Debug, Clone)]
struct SelectorPart {
  kind: String,
  combinator: Option<Combinator>,
}

fn parse_selector_parts(selector: &str) -> Vec<SelectorPart> {
  let mut parts = Vec::new();
  let mut current_part = String::new();
  let mut chars = selector.chars().peekable();
  
  while let Some(ch) = chars.next() {
    match ch {
      ' ' => {
        if !current_part.is_empty() {
          skip_spaces(&mut chars);
          if detect_combinator(&mut chars, '>') {
            parts.push(SelectorPart {
              kind: current_part.trim().to_string(),
              combinator: None,
            });
          } else {
            parts.push(SelectorPart {
              kind: current_part.trim().to_string(),
              combinator: Some(Combinator::Descendant),
            });
          }
          current_part.clear();
        } else {
          skip_spaces(&mut chars);
        }
      }
      '>' => {
        if !current_part.is_empty() {
          parts.push(SelectorPart {
            kind: current_part.trim().to_string(),
            combinator: Some(Combinator::Child),
          });
          current_part.clear();
        } else if let Some(last_part) = parts.last_mut() {
          // Update the last part to have child combinator
          last_part.combinator = Some(Combinator::Child);
        }
        
        // Skip spaces after >
        while chars.peek() == Some(&' ') {
          chars.next();
        }
      }
      _ => {
        current_part.push(ch);
      }
    }
  }
  
  // Add the last part
  if !current_part.is_empty() {
    parts.push(SelectorPart {
      kind: current_part.trim().to_string(),
      combinator: None,
    });
  }
  
  parts
}

fn convert_parts_to_rule<L: Language>(
  parts: Vec<SelectorPart>,
  env: &DeserializeEnv<L>,
) -> Result<Rule, RuleSerializeError> {
  if parts.is_empty() {
    return Err(RuleSerializeError::MissPositiveMatcher);
  }
  
  // The rightmost part is the target node
  let target = &parts[parts.len() - 1];
  let mut rule = Rule::Kind(KindMatcher::try_new(&target.kind, env.lang.clone())?);
  
  // Build nested inside rules from right to left
  for i in (0..parts.len() - 1).rev() {
    let part = &parts[i];
    
    match part.combinator {
      Some(Combinator::Child) => {
        // Direct child relationship
        rule = Rule::Inside(Box::new(Inside::try_new(
          Relation {
            rule: SerializableRule {
              kind: Maybe::Present(part.kind.clone()),
              ..Default::default()
            },
            stop_by: Default::default(),
            field: None,
          },
          env,
        )?));
      }
      Some(Combinator::Descendant) | None => {
        // Descendant relationship (or no combinator, treated as descendant)
        rule = Rule::Inside(Box::new(Inside::try_new(
          Relation {
            rule: SerializableRule {
              kind: Maybe::Present(part.kind.clone()),
              ..Default::default()
            },
            stop_by: Maybe::Nothing, // Allow matching any ancestor
            field: None,
          },
          env,
        )?));
      }
    }
  }
  
  Ok(rule)
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript;
  use ast_grep_core::tree_sitter::LanguageExt;
  
  #[test]
  fn test_parse_selector_simple() {
    let parts = parse_selector_parts("number");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].kind, "number");
    assert!(parts[0].combinator.is_none());
  }
  
  #[test]
  fn test_parse_selector_child() {
    let parts = parse_selector_parts("arguments > number");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].kind, "arguments");
    assert!(matches!(parts[0].combinator, Some(Combinator::Child)));
    assert_eq!(parts[1].kind, "number");
    assert!(parts[1].combinator.is_none());
  }
  
  #[test]
  fn test_parse_selector_complex() {
    let parts = parse_selector_parts("call_expression > arguments > number");
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].kind, "call_expression");
    assert!(matches!(parts[0].combinator, Some(Combinator::Child)));
    assert_eq!(parts[1].kind, "arguments");
    assert!(matches!(parts[1].combinator, Some(Combinator::Child)));
    assert_eq!(parts[2].kind, "number");
    assert!(parts[2].combinator.is_none());
  }
  
  #[test]
  fn test_parse_selector_descendant() {
    let parts = parse_selector_parts("function_declaration number");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].kind, "function_declaration");
    assert!(matches!(parts[0].combinator, Some(Combinator::Descendant)));
    assert_eq!(parts[1].kind, "number");
    assert!(parts[1].combinator.is_none());
  }
  
  #[test]
  fn test_convert_simple_selector() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let rule = parse_selector("number", &env).expect("should parse");
    
    let grep = TypeScript::Tsx.ast_grep("123");
    assert!(grep.root().find(&rule).is_some());
    
    let grep = TypeScript::Tsx.ast_grep("'string'");
    assert!(grep.root().find(&rule).is_none());
  }
  
  #[test]
  fn test_convert_complex_selector_debug() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    
    // Let's test step by step
    // First, just "number"
    let simple_rule = parse_selector("number", &env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("123");
    assert!(grep.root().find(&simple_rule).is_some(), "Simple number should match");
    
    // Now "arguments > number"
    let args_rule = parse_selector("arguments > number", &env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("test(123)");
    assert!(grep.root().find(&args_rule).is_some(), "arguments > number should match in test(123)");
    
    let grep = TypeScript::Tsx.ast_grep("123");
    assert!(grep.root().find(&args_rule).is_none(), "arguments > number should NOT match standalone 123");
    
    // Finally the full rule
    let full_rule = parse_selector("call_expression > arguments > number", &env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("test(123)");
    assert!(grep.root().find(&full_rule).is_some(), "Full rule should match test(123)");
    
    let grep = TypeScript::Tsx.ast_grep("123");
    if grep.root().find(&full_rule).is_some() {
      // This is the failing case, so let's understand why
      println!("WARNING: Full rule matched standalone 123, which shouldn't happen");
    } else {
      println!("GOOD: Full rule did not match standalone 123");
    }
  }
  
  #[test]
  fn test_convert_complex_selector() {
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let rule = parse_selector("call_expression > arguments > number", &env).expect("should parse");
    
    // Test 1: Should match test(123)
    let grep = TypeScript::Tsx.ast_grep("test(123)");
    assert!(grep.root().find(&rule).is_some());
    
    // Test 3: Should NOT match test('string')
    let grep = TypeScript::Tsx.ast_grep("test('string')");
    assert!(grep.root().find(&rule).is_none());
    
    // Test 2: Should NOT match just 123
    // Let's create a fresh rule for this test to avoid any potential ownership issues
    let fresh_rule = parse_selector("call_expression > arguments > number", &env).expect("should parse");
    let grep = TypeScript::Tsx.ast_grep("123");
    let result = grep.root().find(&fresh_rule);
    assert!(result.is_none(), "Rule should not match standalone number");
  }
}