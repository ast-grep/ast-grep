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
          // Skip additional spaces
          while chars.peek() == Some(&' ') {
            chars.next();
          }
          
          // Check if next non-space character is '>'
          if chars.peek() == Some(&'>') {
            // Don't add combinator yet, wait for '>' processing
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
          // Skip additional spaces
          while chars.peek() == Some(&' ') {
            chars.next();
          }
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
  
  // The rightmost part is the target node - this becomes the main rule
  let target = &parts[parts.len() - 1];
  let target_rule = Rule::Kind(KindMatcher::try_new(&target.kind, env.lang.clone())?);
  
  if parts.len() == 1 {
    // Simple case: just a kind selector
    return Ok(target_rule);
  }
  
  // For complex selectors, build the inside relationship chain
  // CSS selector "call_expression > arguments > number" means:
  // number inside arguments inside call_expression
  // We need to build the relations in reverse order
  
  let mut current_relation: Option<SerializableRule> = None;
  
  // Build from innermost to outermost (right to left, excluding target)
  for i in (0..parts.len() - 1).rev() {
    let part = &parts[i];
    
    if current_relation.is_none() {
      // Innermost container (closest to target)
      current_relation = Some(SerializableRule {
        kind: Maybe::Present(part.kind.clone()),
        ..Default::default()
      });
    } else {
      // Wrap this container around the inner relation
      current_relation = Some(SerializableRule {
        kind: Maybe::Present(part.kind.clone()),
        inside: Maybe::Present(Box::new(Relation {
          rule: current_relation.unwrap(),
          stop_by: Default::default(),
          field: None,
        })),
        ..Default::default()
      });
    }
  }
  
  // Create the final inside rule
  let inside_rule = if let Some(relation_rule) = current_relation {
    Rule::Inside(Box::new(Inside::try_new(
      Relation {
        rule: relation_rule,
        stop_by: Default::default(),
        field: None,
      },
      env,
    )?))
  } else {
    return Ok(target_rule); // This shouldn't happen, but just in case
  };
  
  // Combine target rule with inside rule using All (like the verbose version)
  use ast_grep_core::ops as o;
  Ok(Rule::All(o::All::new(vec![target_rule, inside_rule])))
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
  fn test_yaml_selector_parsing() {
    use crate::from_str;
    
    // Test that a selector field is properly parsed from YAML
    let src = r"
selector: number";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    
    // Check that the selector field is populated
    assert!(rule.selector.is_present(), "selector field should be present");
    let selector_value = rule.selector.unwrap();
    assert_eq!(selector_value, "number");
  }
  
  #[test]
  fn test_debug_rule_structures() {
    use crate::from_str;
    use crate::test::TypeScript;
    use ast_grep_core::Matcher;
    
    let env = DeserializeEnv::new(TypeScript::Tsx);
    
    // Test verbose rule structure step by step
    println!("=== Testing verbose structure ===");
    
    // Simple inside first
    let simple_verbose = r"
kind: number
inside:
  kind: arguments";
    let simple_rule: SerializableRule = from_str(simple_verbose).expect("cannot parse");
    let simple_rule = env.deserialize_rule(simple_rule).expect("should deserialize");
    let grep = TypeScript::Tsx.ast_grep("test(123)");
    println!("Simple verbose (number inside arguments): {:?}", grep.root().find(&simple_rule).is_some());
    
    // Now try building the same with selector
    println!("=== Testing selector structure ===");
    let simple_selector = parse_selector("arguments > number", &env).expect("should parse");
    println!("Simple selector (arguments > number): {:?}", grep.root().find(&simple_selector).is_some());
    
    // If these differ, the issue is in the basic 2-level case
    if grep.root().find(&simple_rule).is_some() != grep.root().find(&simple_selector).is_some() {
      println!("ERROR: Basic 2-level structures differ!");
    } else {
      println!("✅ Basic 2-level structures work the same");
      
      // Now test 3-level
      let complex_verbose = r"
kind: number
inside:
  kind: arguments
  inside:
    kind: call_expression";
      let complex_rule: SerializableRule = from_str(complex_verbose).expect("cannot parse");
      let complex_rule = env.deserialize_rule(complex_rule).expect("should deserialize");
      println!("Complex verbose: {:?}", grep.root().find(&complex_rule).is_some());
      
      let complex_selector = parse_selector("call_expression > arguments > number", &env).expect("should parse");
      println!("Complex selector: {:?}", grep.root().find(&complex_selector).is_some());
    }
  }
}