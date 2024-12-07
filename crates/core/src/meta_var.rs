use crate::match_tree::does_node_match_exactly;
use crate::matcher::Matcher;
use crate::source::Content;
use crate::{Doc, Language, Node, StrDoc};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::replacer::formatted_slice;

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

  pub fn insert(&mut self, id: &str, ret: Node<'tree, D>) -> Option<&mut Self> {
    if self.match_variable(id, &ret) {
      self.single_matched.insert(id.to_string(), ret);
      Some(self)
    } else {
      None
    }
  }

  pub fn insert_multi(&mut self, id: &str, ret: Vec<Node<'tree, D>>) -> Option<&mut Self> {
    if self.match_multi_var(id, &ret) {
      self.multi_matched.insert(id.to_string(), ret);
      Some(self)
    } else {
      None
    }
  }

  pub fn insert_transformation(&mut self, var: &MetaVariable, name: &str, slice: Underlying<D>) {
    let node = match var {
      MetaVariable::Capture(v, _) => self.single_matched.get(v),
      MetaVariable::MultiCapture(vs) => self.multi_matched.get(vs).and_then(|vs| vs.first()),
      _ => None,
    };
    let deindented = if let Some(v) = node {
      formatted_slice(&slice, v.root.doc.get_source(), v.range().start).to_vec()
    } else {
      slice
    };
    self.transformed_var.insert(name.to_string(), deindented);
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
      .or_default()
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
      .map(|n| MetaVariable::Capture(n, false));
    let transformed = self
      .transformed_var
      .keys()
      .cloned()
      .map(|n| MetaVariable::Capture(n, false));
    let multi = self
      .multi_matched
      .keys()
      .cloned()
      .map(MetaVariable::MultiCapture);
    single.chain(multi).chain(transformed)
  }

  pub fn match_constraints<M: Matcher<D::Lang>>(
    &mut self,
    var_matchers: &HashMap<MetaVariableID, M>,
  ) -> bool {
    let mut env = Cow::Borrowed(self);
    for (var_id, candidate) in &self.single_matched {
      if let Some(m) = var_matchers.get(var_id) {
        if m.match_node_with_env(candidate.clone(), &mut env).is_none() {
          return false;
        }
      }
    }
    if let Cow::Owned(env) = env {
      *self = env;
    }
    true
  }

  fn match_variable(&self, id: &str, candidate: &Node<D>) -> bool {
    if let Some(m) = self.single_matched.get(id) {
      return does_node_match_exactly(m, candidate);
    }
    true
  }
  fn match_multi_var(&self, id: &str, cands: &[Node<D>]) -> bool {
    let Some(nodes) = self.multi_matched.get(id) else {
      return true;
    };
    let mut named_nodes = nodes.iter().filter(|n| n.is_named());
    let mut named_cands = cands.iter().filter(|n| n.is_named());
    loop {
      if let Some(node) = named_nodes.next() {
        let Some(cand) = named_cands.next() else {
          // cand is done but node is not
          break false;
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

  /// internal for readopt NodeMatch in pinned.rs
  /// readopt node and env when sending them to other threads
  pub(crate) fn visit_nodes<F>(&mut self, mut f: F)
  where
    F: FnMut(&mut Node<'_, D>),
  {
    for n in self.single_matched.values_mut() {
      f(n)
    }
    for ns in self.multi_matched.values_mut() {
      for n in ns {
        f(n)
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
    MetaVariable::Capture(n, _) => {
      if let Some(node) = env.get_match(n) {
        let bytes = node.root.doc.get_source().get_range(node.range());
        Some(bytes)
      } else if let Some(bytes) = env.get_transformed(n) {
        Some(bytes)
      } else {
        None
      }
    }
    MetaVariable::MultiCapture(n) => {
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

impl<D: Doc> Default for MetaVarEnv<'_, D> {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaVariable {
  /// $A for captured meta var
  Capture(MetaVariableID, bool),
  /// $_ for non-captured meta var
  Dropped(bool),
  /// $$$ for non-captured multi var
  Multiple,
  /// $$$A for captured ellipsis
  MultiCapture(MetaVariableID),
}

pub(crate) fn extract_meta_var(src: &str, meta_char: char) -> Option<MetaVariable> {
  use MetaVariable::*;
  let ellipsis: String = std::iter::repeat(meta_char).take(3).collect();
  if src == ellipsis {
    return Some(Multiple);
  }
  if let Some(trimmed) = src.strip_prefix(&ellipsis) {
    if !trimmed.chars().all(is_valid_meta_var_char) {
      return None;
    }
    if trimmed.starts_with('_') {
      return Some(Multiple);
    } else {
      return Some(MultiCapture(trimmed.to_owned()));
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
  if !trimmed.starts_with(is_valid_first_char) || // empty or started with number
    !trimmed.chars().all(is_valid_meta_var_char)
  // not in form of $A or $_
  {
    return None;
  }
  if trimmed.starts_with('_') {
    Some(Dropped(named))
  } else {
    Some(Capture(trimmed.to_owned(), named))
  }
}

#[inline]
fn is_valid_first_char(c: char) -> bool {
  matches!(c, 'A'..='Z' | '_')
}

#[inline]
pub(crate) fn is_valid_meta_var_char(c: char) -> bool {
  is_valid_first_char(c) || c.is_ascii_digit()
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
  use crate::Pattern;

  fn extract_var(s: &str) -> Option<MetaVariable> {
    extract_meta_var(s, '$')
  }
  #[test]
  fn test_match_var() {
    use MetaVariable::*;
    assert_eq!(extract_var("$$$"), Some(Multiple));
    assert_eq!(extract_var("$ABC"), Some(Capture("ABC".into(), true)));
    assert_eq!(extract_var("$$ABC"), Some(Capture("ABC".into(), false)));
    assert_eq!(extract_var("$MATCH1"), Some(Capture("MATCH1".into(), true)));
    assert_eq!(extract_var("$$$ABC"), Some(MultiCapture("ABC".into())));
    assert_eq!(extract_var("$_"), Some(Dropped(true)));
    assert_eq!(extract_var("$_123"), Some(Dropped(true)));
    assert_eq!(extract_var("$$_"), Some(Dropped(false)));
  }

  #[test]
  fn test_not_meta_var() {
    assert_eq!(extract_var("$123"), None);
    assert_eq!(extract_var("$"), None);
    assert_eq!(extract_var("$$"), None);
    assert_eq!(extract_var("abc"), None);
    assert_eq!(extract_var("$abc"), None);
  }

  fn match_constraints(pattern: &str, node: &str) -> bool {
    let mut matchers = HashMap::new();
    matchers.insert("A".to_string(), Pattern::new(pattern, Tsx));
    let mut env = MetaVarEnv::new();
    let root = Tsx.ast_grep(node);
    let node = root.root().child(0).unwrap().child(0).unwrap();
    env.insert("A", node);
    env.match_constraints(&matchers)
  }

  #[test]
  fn test_non_ascii_meta_var() {
    let extract = |s| extract_meta_var(s, 'µ');
    use MetaVariable::*;
    assert_eq!(extract("µµµ"), Some(Multiple));
    assert_eq!(extract("µABC"), Some(Capture("ABC".into(), true)));
    assert_eq!(extract("µµABC"), Some(Capture("ABC".into(), false)));
    assert_eq!(extract("µµµABC"), Some(MultiCapture("ABC".into())));
    assert_eq!(extract("µ_"), Some(Dropped(true)));
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
