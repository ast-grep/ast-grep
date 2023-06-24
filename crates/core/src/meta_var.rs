use crate::match_tree::does_node_match_exactly;
use crate::matcher::{KindMatcher, Pattern, RegexMatcher};
use crate::source::Content;
use crate::{Doc, Language, Node, StrDoc};
use std::borrow::Cow;
use std::collections::HashMap;

pub type MetaVariableID = String;

type Underlying<D> = Vec<<<D as Doc>::Source as Content>::Underlying>;
/// a dictionary that stores metavariable instantiation
/// const a = 123 matched with const a = $A will produce env: $A => 123
#[derive(Clone)]
pub struct MetaVarEnv<'tree, D: Doc> {
  single_matched: HashMap<MetaVariableID, Node<'tree, D>>,
  multi_matched: HashMap<MetaVariableID, Vec<Node<'tree, D>>>,
  transformed_var: HashMap<MetaVariableID, Underlying<D>>,
}

impl<'tree, D: Doc> MetaVarEnv<'tree, D> {
  pub fn new() -> Self {
    Self {
      single_matched: HashMap::new(),
      multi_matched: HashMap::new(),
      transformed_var: HashMap::new(),
    }
  }

  pub fn insert(&mut self, id: MetaVariableID, ret: Node<'tree, D>) -> Option<&mut Self> {
    if self.match_variable(&id, &ret) {
      self.single_matched.insert(id, ret);
      Some(self)
    } else {
      None
    }
  }

  pub fn insert_multi(
    &mut self,
    id: MetaVariableID,
    ret: Vec<Node<'tree, D>>,
  ) -> Option<&mut Self> {
    if self.match_multi_var(&id, &ret) {
      self.multi_matched.insert(id, ret);
      Some(self)
    } else {
      None
    }
  }

  pub fn insert_transformation(&mut self, name: MetaVariableID, src: Underlying<D>) {
    self.transformed_var.insert(name, src);
  }

  pub fn get_match(&self, var: &str) -> Option<&'_ Node<'tree, D>> {
    self.single_matched.get(var)
  }

  pub fn get_multiple_matches(&self, var: &str) -> Vec<Node<'tree, D>> {
    self.multi_matched.get(var).cloned().unwrap_or_default()
  }

  pub fn get_transformed(&self, var: &str) -> Option<&Underlying<D>> {
    self.transformed_var.get(var)
  }
  pub fn get_var_bytes<'s>(
    &'s self,
    var: &MetaVariable,
  ) -> Option<&'s [<D::Source as Content>::Underlying]> {
    get_var_bytes_impl(self, var)
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
    let transformed = self
      .transformed_var
      .keys()
      .cloned()
      .map(|n| MetaVariable::Named(n, false));
    let multi = self
      .multi_matched
      .keys()
      .cloned()
      .map(MetaVariable::NamedEllipsis);
    single.chain(multi).chain(transformed)
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

  fn match_variable(&self, id: &MetaVariableID, candidate: &Node<D>) -> bool {
    if let Some(m) = self.single_matched.get(id) {
      return does_node_match_exactly(m, candidate);
    }
    true
  }
  fn match_multi_var(&self, id: &MetaVariableID, cands: &[Node<D>]) -> bool {
    let Some(nodes) = self.multi_matched.get(id) else {
      return true;
    };
    let mut named_nodes = nodes.iter().filter(|n| n.is_named());
    let mut named_cands = cands.iter().filter(|n| n.is_named());
    loop {
      if let Some(node) = named_nodes.next() {
        let Some(cand) = named_cands.next() else {
          // cand is done but node is not
          break false
        };
        if !does_node_match_exactly(node, cand) {
          break false;
        }
      } else if named_cands.next().is_some() {
        // node is done but cand is not
        break false;
      } else {
        // both None, matches
        break true;
      }
    }
  }
}

fn get_var_bytes_impl<'t, C, D>(
  env: &'t MetaVarEnv<'t, D>,
  var: &MetaVariable,
) -> Option<&'t [C::Underlying]>
where
  D: Doc<Source = C>,
  C: Content + 't,
{
  match var {
    MetaVariable::Named(n, _) => {
      if let Some(node) = env.get_match(n) {
        let bytes = node.root.doc.get_source().get_range(node.range());
        Some(bytes)
      } else if let Some(bytes) = env.get_transformed(n) {
        Some(bytes)
      } else {
        None
      }
    }
    MetaVariable::NamedEllipsis(n) => {
      let nodes = env.get_multiple_matches(n);
      if nodes.is_empty() {
        None
      } else {
        // NOTE: start_byte is not always index range of source's slice.
        // e.g. start_byte is still byte_offset in utf_16 (napi). start_byte
        // so we need to call source's get_range method
        let start = nodes[0].inner.start_byte() as usize;
        let end = nodes[nodes.len() - 1].inner.end_byte() as usize;
        Some(nodes[0].root.doc.get_source().get_range(start..end))
      }
    }
    _ => None,
  }
}

impl<'tree, D: Doc> Default for MetaVarEnv<'tree, D> {
  fn default() -> Self {
    Self::new()
  }
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

pub(crate) fn is_valid_meta_var_char(c: char) -> bool {
  matches!(c, 'A'..='Z' | '_' | '0'..='9')
}

impl<'tree, L: Language> From<MetaVarEnv<'tree, StrDoc<L>>> for HashMap<String, String> {
  fn from(env: MetaVarEnv<'tree, StrDoc<L>>) -> Self {
    let mut ret = HashMap::new();
    for (id, node) in env.single_matched {
      ret.insert(id, node.text().into());
    }
    for (id, bytes) in env.transformed_var {
      ret.insert(
        id,
        String::from_utf8(bytes).expect("invalid transform variable"),
      );
    }
    for (id, nodes) in env.multi_matched {
      let s: Vec<_> = nodes.iter().map(|n| n.text()).collect();
      let s = s.join(", ");
      ret.insert(id, format!("[{s}]"));
    }
    ret
  }
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
    assert_eq!(extract_var("$MATCH1"), Some(Named("MATCH1".into(), true)));
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

  #[test]
  fn test_multi_var_match() {
    let grep = Tsx.ast_grep("if (true) { a += 1; b += 1 } else { a += 1; b += 1 }");
    let node = grep.root();
    let found = node.find("if (true) { $$$A } else { $$$A }");
    assert!(found.is_some());
    let grep = Tsx.ast_grep("if (true) { a += 1 } else { b += 1 }");
    let node = grep.root();
    let not_found = node.find("if (true) { $$$A } else { $$$A }");
    assert!(not_found.is_none());
  }

  #[test]
  fn test_multi_var_match_with_trailing() {
    let grep = Tsx.ast_grep("if (true) { a += 1; } else { a += 1; b += 1 }");
    let node = grep.root();
    let not_found = node.find("if (true) { $$$A } else { $$$A }");
    assert!(not_found.is_none());
    let grep = Tsx.ast_grep("if (true) { a += 1; b += 1; } else { a += 1 }");
    let node = grep.root();
    let not_found = node.find("if (true) { $$$A } else { $$$A }");
    assert!(not_found.is_none());
  }
}
