mod relational_rule;

use relational_rule::{Follows, Has, Inside, Precedes};

pub use crate::constraints::{
  try_deserialize_matchers, try_from_serializable as deserialize_meta_var, RuleWithConstraint,
  SerializableMetaVarMatcher,
};
use ast_grep_core::language::Language;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::meta_var::MetaVarMatchers;
use ast_grep_core::ops as o;
use ast_grep_core::replace_meta_var_in_string;
use ast_grep_core::NodeMatch;
use ast_grep_core::{KindMatcher, Matcher, Node, Pattern};
use globset::Glob;
use globset::GlobSetBuilder;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
  Hint,
  Info,
  Warning,
  Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RuleConfig<L: Language> {
  /// Unique, descriptive identifier, e.g., no-unused-variable
  pub id: String,
  /// Main message highlighting why this rule fired. It should be single line and concise,
  /// but specific enough to be understood without additional context.
  pub message: String,
  /// Additional notes to elaborate the message and provide potential fix to the issue.
  pub note: Option<String>,
  /// One of: Info, Warning, or Error
  pub severity: Severity,
  /// Specify the language to parse and the file extension to includ in matching.
  pub language: L,
  /// Pattern rules to find matching AST nodes
  pub rule: SerializableRule,
  /// A pattern to auto fix the issue. It can reference metavariables appeared in rule.
  pub fix: Option<String>,
  /// Addtional meta variables pattern to filter matching
  pub constraints: Option<HashMap<String, SerializableMetaVarMatcher>>,
  /// Glob patterns to specify that the rule only applies to matching files
  pub files: Option<Vec<String>>,
  /// Glob patterns that exclude rules from applying to files
  pub ignores: Option<Vec<String>>,
  /// Documentation link to this rule
  pub url: Option<String>,
  /// Extra information for the rule
  pub metadata: Option<HashMap<String, String>>,
}

impl<L: Language> RuleConfig<L> {
  pub fn get_matcher(&self) -> RuleWithConstraint<L> {
    let rule = self.get_rule();
    let matchers = self.get_meta_var_matchers();
    RuleWithConstraint { rule, matchers }
  }

  pub fn get_rule(&self) -> Rule<L> {
    try_from_serializable(self.rule.clone(), self.language.clone()).unwrap()
  }

  pub fn get_fixer(&self) -> Option<Pattern<L>> {
    Some(Pattern::new(self.fix.as_ref()?, self.language.clone()))
  }

  pub fn get_meta_var_matchers(&self) -> MetaVarMatchers<L> {
    if let Some(constraints) = self.constraints.clone() {
      try_deserialize_matchers(constraints, self.language.clone()).unwrap()
    } else {
      MetaVarMatchers::default()
    }
  }

  pub fn matches_path(&self, path: &Path) -> bool {
    let mut files_glob_set_builder: GlobSetBuilder = GlobSetBuilder::new();
    let mut found_files_glob_conditional = false;
    if let Some(files) = &self.files {
      for path in files {
        if let Ok(glob) = Glob::new(path.as_str()) {
          found_files_glob_conditional = true;
          files_glob_set_builder.add(glob);
        }
      }
    }

    let mut ignores_glob_set_builder: GlobSetBuilder = GlobSetBuilder::new();
    let mut found_ignores_glob_conditional = false;
    if let Some(files) = &self.ignores {
      for path in files {
        if let Ok(glob) = Glob::new(path.as_str()) {
          found_ignores_glob_conditional = true;
          ignores_glob_set_builder.add(glob);
        }
      }
    }

    let files_glob_set = files_glob_set_builder.build().unwrap();
    let ignores_glob_set = ignores_glob_set_builder.build().unwrap();
    let mut should_match = false;

    if found_files_glob_conditional && files_glob_set.is_match(path) {
      should_match = true;
    }

    if !found_files_glob_conditional {
      // true, unless a match is made by the ignores_glob_set.
      should_match = true;
    }

    if found_ignores_glob_conditional && ignores_glob_set.is_match(path) {
      should_match = false;
    }

    should_match
  }

  pub fn get_message(&self, node: &NodeMatch<L>) -> String {
    replace_meta_var_in_string(&self.message, node.get_env(), node.lang())
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SerializableRule {
  All(Vec<SerializableRule>),
  Any(Vec<SerializableRule>),
  Not(Box<SerializableRule>),
  Inside(Box<RelationalRule>),
  Has(Box<RelationalRule>),
  Precedes(Box<RelationalRule>),
  Follows(Box<RelationalRule>),
  Pattern(PatternStyle),
  Kind(String),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelationalRule {
  #[serde(flatten)]
  rule: SerializableRule,
  #[serde(default)]
  until: Option<SerializableRule>,
  #[serde(default)]
  immediate: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PatternStyle {
  Str(String),
  Contextual { context: String, selector: String },
}

pub enum Rule<L: Language> {
  All(o::All<L, Rule<L>>),
  Any(o::Any<L, Rule<L>>),
  Not(Box<o::Not<L, Rule<L>>>),
  Inside(Box<Inside<L>>),
  Has(Box<Has<L>>),
  Precedes(Box<Precedes<L>>),
  Follows(Box<Follows<L>>),
  Pattern(Pattern<L>),
  Kind(KindMatcher<L>),
}

impl<L: Language> Matcher<L> for Rule<L> {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    use Rule::*;
    match self {
      All(all) => all.match_node_with_env(node, env),
      Any(any) => any.match_node_with_env(node, env),
      Not(not) => not.match_node_with_env(node, env),
      Inside(parent) => match_and_add_label(&**parent, node, env),
      Has(child) => match_and_add_label(&**child, node, env),
      Precedes(latter) => match_and_add_label(&**latter, node, env),
      Follows(former) => match_and_add_label(&**former, node, env),
      Pattern(pattern) => pattern.match_node_with_env(node, env),
      Kind(kind) => kind.match_node_with_env(node, env),
    }
  }
}
fn match_and_add_label<'tree, L: Language, M: Matcher<L>>(
  inner: &M,
  node: Node<'tree, L>,
  env: &mut MetaVarEnv<'tree, L>,
) -> Option<Node<'tree, L>> {
  let matched = inner.match_node_with_env(node, env)?;
  env.add_label("secondary", matched.clone());
  Some(matched)
}

#[derive(Debug)]
pub enum SerializeError {
  MissPositiveMatcher,
}

impl std::error::Error for SerializeError {}
impl fmt::Display for SerializeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::MissPositiveMatcher => write!(f, "missing positive matcher"),
    }
  }
}

// TODO: implement positive/non positive
pub fn try_from_serializable<L: Language>(
  serialized: SerializableRule,
  lang: L,
) -> Result<Rule<L>, SerializeError> {
  use Rule as R;
  use SerializableRule as S;
  let mapper = |s| try_from_serializable(s, lang.clone());
  let convert_rules = |rules: Vec<SerializableRule>| {
    let mut inner = Vec::with_capacity(rules.len());
    for rule in rules {
      inner.push(try_from_serializable(rule, lang.clone())?);
    }
    Ok(inner)
  };
  let ret = match serialized {
    S::All(all) => R::All(o::All::new(convert_rules(all)?)),
    S::Any(any) => R::Any(o::Any::new(convert_rules(any)?)),
    S::Not(not) => R::Not(Box::new(o::Not::new(mapper(*not)?))),
    S::Inside(inside) => R::Inside(Box::new(Inside::try_new(*inside, lang)?)),
    S::Has(has) => R::Has(Box::new(Has::try_new(*has, lang)?)),
    S::Precedes(precedes) => R::Precedes(Box::new(Precedes::try_new(*precedes, lang)?)),
    S::Follows(follows) => R::Follows(Box::new(Follows::try_new(*follows, lang)?)),
    S::Kind(kind) => R::Kind(KindMatcher::new(&kind, lang)),
    S::Pattern(PatternStyle::Str(pattern)) => R::Pattern(Pattern::new(&pattern, lang)),
    S::Pattern(PatternStyle::Contextual { context, selector }) => {
      R::Pattern(Pattern::contextual(&context, &selector, lang))
    }
  };
  Ok(ret)
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;
  use crate::PatternStyle::*;
  use SerializableRule as S;

  #[test]
  fn test_pattern() {
    let src = r"
pattern: Test
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(matches!(rule, S::Pattern(Str(_))));
    let src = r"
pattern:
    context: class $C { set $B() {} }
    selector: method_definition
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    assert!(matches!(rule, S::Pattern(Contextual { .. })));
  }

  #[test]
  fn test_relational() {
    let src = r"
inside:
    pattern: class A {}
    immediate: true
    until:
        pattern: function() {}
";
    let rule: SerializableRule = from_str(src).expect("cannot parse rule");
    match rule {
      SerializableRule::Inside(rule) => assert!(rule.immediate),
      _ => unreachable!(),
    }
  }

  fn ts_rule_config(rule: SerializableRule) -> RuleConfig<TypeScript> {
    RuleConfig {
      id: "".into(),
      message: "".into(),
      note: None,
      severity: Severity::Hint,
      language: TypeScript::Tsx,
      rule,
      fix: None,
      constraints: None,
      files: None,
      ignores: None,
      url: None,
      metadata: None,
    }
  }

  #[test]
  fn test_rule_message() {
    let rule = from_str("pattern: class $A {}").expect("cannot parse rule");
    let config = RuleConfig {
      id: "test".into(),
      message: "Found $A".into(),
      ..ts_rule_config(rule)
    };
    let grep = TypeScript::Tsx.ast_grep("class TestClass {}");
    let node_match = grep
      .root()
      .find(config.get_matcher())
      .expect("should find match");
    assert_eq!(config.get_message(&node_match), "Found TestClass");
  }
}
