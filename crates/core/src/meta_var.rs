use crate::match_tree::does_node_match_exactly;
use crate::matcher::{KindMatcher, Pattern, RegexMatcher};
use crate::{Doc, Node};
use std::borrow::Cow;
use std::collections::HashMap;

pub type MetaVariableID = String;

/// a dictionary that stores metavariable instantiation
/// const a = 123 matched with const a = $A will produce env: $A => 123
#[derive(Clone)]
pub struct MetaVarEnv<'tree, D: Doc> {
  single_matched: HashMap<MetaVariableID, Node<'tree, D>>,
  multi_matched: HashMap<MetaVariableID, Vec<Node<'tree, D>>>,
}

impl<'tree, D: Doc> MetaVarEnv<'tree, D> {
  pub fn new() -> Self {
    Self {
      single_matched: HashMap::new(),
      multi_matched: HashMap::new(),
    }
  }

  pub fn insert(&mut self, id: MetaVariableID, ret: Node<'tree, D>) -> Option<&mut Self> {
    if !self.match_variable(&id, ret.clone()) {
      return None;
    }
    self.single_matched.insert(id, ret);
    Some(self)
  }

  pub fn insert_multi(
    &mut self,
    id: MetaVariableID,
    ret: Vec<Node<'tree, D>>,
  ) -> Option<&mut Self> {
    self.multi_matched.insert(id, ret);
    Some(self)
  }

  pub fn get(&self, var: &MetaVariable) -> Option<MatchResult<'_, 'tree, D>> {
    match var {
      MetaVariable::Named(n, _) => self.single_matched.get(n).map(MatchResult::Single),
      MetaVariable::NamedEllipsis(n) => self.multi_matched.get(n).map(MatchResult::Multi),
      _ => None,
    }
  }

  pub fn get_match(&self, var: &str) -> Option<&'_ Node<'tree, D>> {
    self.single_matched.get(var)
  }

  pub fn get_multiple_matches(&self, var: &str) -> Vec<Node<'tree, D>> {
    self.multi_matched.get(var).cloned().unwrap_or_default()
  }

  pub fn add_label(&mut self, label: &str, node: Node<'tree, D>) {
    self
      .multi_matched
      .entry(label.into())
      .or_insert_with(Vec::new)
      .push(node);
  }

  pub fn get_labels(&self, label: &str) -> Option<&Vec<Node<'tree, D>>> {
    self.multi_matched.get(label)
  }

  pub fn get_matched_variables(&self) -> impl Iterator<Item = MetaVariable> + '_ {
    let single = self
      .single_matched
      .keys()
      .cloned()
      .map(|n| MetaVariable::Named(n, false));
    let multi = self
      .multi_matched
      .keys()
      .cloned()
      .map(MetaVariable::NamedEllipsis);
    single.chain(multi)
  }

  pub fn match_constraints(
    &self,
    var_matchers: &MetaVarMatchers<impl Doc<Lang = D::Lang>>,
  ) -> bool {
    for (var_id, candidate) in &self.single_matched {
      if let Some(m) = var_matchers.0.get(var_id) {
        if !m.matches(candidate.clone()) {
          return false;
        }
      }
    }
    true
  }

  fn match_variable(&self, id: &MetaVariableID, candidate: Node<D>) -> bool {
    if let Some(m) = self.single_matched.get(id) {
      return does_node_match_exactly(m, candidate);
    }
    true
  }
}

impl<'tree, D: Doc> Default for MetaVarEnv<'tree, D> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'tree, D: Doc> From<MetaVarEnv<'tree, D>> for HashMap<String, String> {
  fn from(env: MetaVarEnv<'tree, D>) -> Self {
    let mut ret = HashMap::new();
    for (id, node) in env.single_matched {
      ret.insert(id, node.text().into());
    }
    for (id, nodes) in env.multi_matched {
      let s: Vec<_> = nodes.iter().map(|n| n.text()).collect();
      let s = s.join(", ");
      ret.insert(id, format!("[{s}]"));
    }
    ret
  }
}

pub enum MatchResult<'a, 'tree, D: Doc> {
  /// $A for captured meta var
  Single(&'a Node<'tree, D>),
  /// $$$A for captured ellipsis
  Multi(&'a Vec<Node<'tree, D>>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum MetaVariable {
  /// $A for captured meta var
  Named(MetaVariableID, bool),
  /// $_ for non-captured meta var
  Anonymous(bool),
  /// $$$ for non-captured ellipsis
  Ellipsis,
  /// $$$A for captured ellipsis
  NamedEllipsis(MetaVariableID),
}
// TODO add var_extract
pub enum MetaVarExtract {
  Single(),
  Multiple(),
  // TODO: add transform
  // Transformed(),
}

#[derive(Clone)]
pub struct MetaVarMatchers<D: Doc>(HashMap<MetaVariableID, MetaVarMatcher<D>>);

impl<D: Doc> MetaVarMatchers<D> {
  pub fn new() -> Self {
    Self(HashMap::new())
  }

  pub fn insert(&mut self, var_id: MetaVariableID, matcher: MetaVarMatcher<D>) {
    self.0.insert(var_id, matcher);
  }
}

impl<D: Doc> Default for MetaVarMatchers<D> {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Clone)]
pub enum MetaVarMatcher<D: Doc> {
  #[cfg(feature = "regex")]
  /// A regex to filter matched metavar based on its textual content.
  Regex(RegexMatcher<D::Lang>),
  /// A pattern to filter matched metavar based on its AST tree shape.
  Pattern(Pattern<D>),
  /// A kind_id to filter matched metavar based on its ts-node kind
  Kind(KindMatcher<D::Lang>),
}

impl<D: Doc> MetaVarMatcher<D> {
  pub fn matches(&self, candidate: Node<impl Doc<Lang = D::Lang>>) -> bool {
    use crate::matcher::Matcher;
    use MetaVarMatcher::*;
    let mut env = Cow::Owned(MetaVarEnv::new());
    match self {
      #[cfg(feature = "regex")]
      Regex(r) => r.match_node_with_env(candidate, &mut env).is_some(),
      Pattern(p) => p.match_node_with_env(candidate, &mut env).is_some(),
      Kind(k) => k.match_node_with_env(candidate, &mut env).is_some(),
    }
  }
}

pub(crate) fn extract_meta_var(src: &str, meta_char: char) -> Option<MetaVariable> {
  use MetaVariable::*;
  let ellipsis: String = std::iter::repeat(meta_char).take(3).collect();
  if src == ellipsis {
    return Some(Ellipsis);
  }
  if let Some(trimmed) = src.strip_prefix(&ellipsis) {
    if !trimmed.chars().all(is_valid_meta_var_char) {
      return None;
    }
    if trimmed.starts_with('_') {
      return Some(Ellipsis);
    } else {
      return Some(NamedEllipsis(trimmed.to_owned()));
    }
  }
  if !src.starts_with(meta_char) {
    return None;
  }
  let trimmed = &src[meta_char.len_utf8()..];
  let (trimmed, named) = if let Some(t) = trimmed.strip_prefix(meta_char) {
    (t, false)
  } else {
    (trimmed, true)
  };
  // $A or $_
  if !trimmed.chars().all(is_valid_meta_var_char) {
    return None;
  }
  if trimmed.starts_with('_') {
    Some(Anonymous(named))
  } else {
    Some(Named(trimmed.to_owned(), named))
  }
}

pub fn split_first_meta_var(mut src: &str, meta_char: char) -> Option<(MetaVariable, &str)> {
  debug_assert!(src.starts_with(meta_char));
  let mut i = 0;
  let (trimmed, is_multi) = loop {
    i += 1;
    src = &src[meta_char.len_utf8()..];
    if i == 3 {
      break (src, true);
    }
    if !src.starts_with(meta_char) {
      break (src, false);
    }
  };
  // no Anonymous meta var allowed, so _ is not allowed
  let i = trimmed
    .find(|c: char| !c.is_ascii_uppercase())
    .unwrap_or(trimmed.len());
  // no name found
  if i == 0 {
    return None;
  }
  let name = trimmed[..i].to_string();
  let var = if is_multi {
    MetaVariable::NamedEllipsis(name)
  } else {
    MetaVariable::Named(name, true)
  };
  Some((var, &trimmed[i..]))
}

fn is_valid_meta_var_char(c: char) -> bool {
  matches!(c, 'A'..='Z' | '_')
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};
  use crate::{Pattern, StrDoc};

  fn extract_var(s: &str) -> Option<MetaVariable> {
    extract_meta_var(s, '$')
  }
  #[test]
  fn test_match_var() {
    use MetaVariable::*;
    assert_eq!(extract_var("$$$"), Some(Ellipsis));
    assert_eq!(extract_var("$ABC"), Some(Named("ABC".into(), true)));
    assert_eq!(extract_var("$$ABC"), Some(Named("ABC".into(), false)));
    assert_eq!(extract_var("$$$ABC"), Some(NamedEllipsis("ABC".into())));
    assert_eq!(extract_var("$_"), Some(Anonymous(true)));
    assert_eq!(extract_var("abc"), None);
    assert_eq!(extract_var("$abc"), None);
  }

  fn match_constraints(pattern: &str, node: &str) -> bool {
    let mut matchers: MetaVarMatchers<StrDoc<_>> = MetaVarMatchers(HashMap::new());
    matchers.insert(
      "A".to_string(),
      MetaVarMatcher::Pattern(Pattern::new(pattern, Tsx)),
    );
    let mut env = MetaVarEnv::new();
    let root = Tsx.ast_grep(node);
    let node = root.root().child(0).unwrap().child(0).unwrap();
    env.insert("A".to_string(), node);
    env.match_constraints(&matchers)
  }

  #[test]
  fn test_non_ascii_meta_var() {
    let extract = |s| extract_meta_var(s, 'µ');
    use MetaVariable::*;
    assert_eq!(extract("µµµ"), Some(Ellipsis));
    assert_eq!(extract("µABC"), Some(Named("ABC".into(), true)));
    assert_eq!(extract("µµABC"), Some(Named("ABC".into(), false)));
    assert_eq!(extract("µµµABC"), Some(NamedEllipsis("ABC".into())));
    assert_eq!(extract("µ_"), Some(Anonymous(true)));
    assert_eq!(extract("abc"), None);
    assert_eq!(extract("µabc"), None);
  }

  #[test]
  fn test_match_constraints() {
    assert!(match_constraints("a + b", "a + b"));
  }

  #[test]
  fn test_match_not_constraints() {
    assert!(!match_constraints("a - b", "a + b"));
  }
}
