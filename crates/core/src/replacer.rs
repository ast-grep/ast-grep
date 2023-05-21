use crate::language::Language;
use crate::meta_var::{split_first_meta_var, MatchResult, MetaVarEnv};
use crate::source::{Content, Edit as E};
use crate::Pattern;
use crate::{Doc, Node, Root};

type Edit<D> = E<<D as Doc>::Source>;

type Underlying<S> = Vec<<S as Content>::Underlying>;

/// Replace meta variable in the replacer string
pub trait Replacer<D: Doc> {
  fn generate_replacement(&self, env: &MetaVarEnv<D>, lang: D::Lang) -> Underlying<D::Source>;
}

impl<D: Doc> Replacer<D> for str {
  fn generate_replacement(&self, env: &MetaVarEnv<D>, lang: D::Lang) -> Underlying<D::Source> {
    let root = Root::new(self, lang.clone());
    let edits = collect_edits(&root, env, lang);
    merge_edits_to_vec(edits, &root)
  }
}

impl<D: Doc> Replacer<D> for Pattern<D> {
  fn generate_replacement(&self, env: &MetaVarEnv<D>, lang: D::Lang) -> Underlying<D::Source> {
    let edits = collect_edits(&self.root, env, lang);
    merge_edits_to_vec(edits, &self.root)
  }
}

impl<D, T> Replacer<D> for &T
where
  D: Doc,
  T: Replacer<D> + ?Sized,
{
  fn generate_replacement(&self, env: &MetaVarEnv<D>, lang: D::Lang) -> Underlying<D::Source> {
    (**self).generate_replacement(env, lang)
  }
}

fn collect_edits<D: Doc>(root: &Root<D>, env: &MetaVarEnv<D>, lang: D::Lang) -> Vec<Edit<D>> {
  let mut node = root.root();
  let root_id = node.inner.id();
  let mut edits = vec![];

  // this is a post-order DFS that stops traversal when the node matches
  'outer: loop {
    if let Some(text) = get_meta_var_replacement(&node, env, lang.clone()) {
      let position = node.inner.start_byte();
      let length = node.inner.end_byte() - position;
      edits.push(Edit::<D> {
        position: position as usize,
        deleted_length: length as usize,
        inserted_text: text,
      });
    } else if let Some(first_child) = node.child(0) {
      // traverse down to child
      node = first_child;
      continue;
    } else if node.inner.is_missing() {
      // TODO: better handling missing node
      if let Some(sibling) = node.next() {
        node = sibling;
        continue;
      } else {
        break;
      }
    }
    // traverse up to parent until getting to root
    loop {
      // come back to the root node, terminating dfs
      if node.inner.id() == root_id {
        break 'outer;
      }
      if let Some(sibling) = node.next() {
        node = sibling;
        break;
      }
      node = node.parent().unwrap();
    }
  }
  // add the missing one
  edits.push(Edit::<D> {
    position: root.root().range().end,
    deleted_length: 0,
    inserted_text: vec![],
  });
  edits
}

// replace meta_var in template string, e.g. "Hello $NAME" -> "Hello World"
// TODO: use Cow instead of String
pub fn replace_meta_var_in_string<D: Doc>(
  mut template: &str,
  env: &MetaVarEnv<D>,
  lang: &D::Lang,
) -> String {
  let mv_char = lang.meta_var_char();
  let mut ret = String::new();
  while let Some(i) = template.find(mv_char) {
    ret.push_str(&template[..i]);
    template = &template[i..];
    let (meta_var, remaining) = split_first_meta_var(template, mv_char);
    if let Some(n) = env.get_match(meta_var) {
      ret.push_str(&n.text());
    }
    template = remaining;
  }
  ret.push_str(template);
  ret
}

fn merge_edits_to_vec<D: Doc>(edits: Vec<Edit<D>>, root: &Root<D>) -> Underlying<D::Source> {
  let mut ret = vec![];
  let mut start = 0;
  for edit in edits {
    debug_assert!(start <= edit.position, "Edit must be ordered!");
    ret.extend(
      root
        .doc
        .get_source()
        .get_range(start..edit.position)
        .iter()
        .cloned(),
    );
    ret.extend(edit.inserted_text.iter().cloned());
    start = edit.position + edit.deleted_length;
  }
  ret
}

fn get_meta_var_replacement<D: Doc>(
  node: &Node<D>,
  env: &MetaVarEnv<D>,
  lang: D::Lang,
) -> Option<Underlying<D::Source>> {
  if !node.is_named_leaf() {
    return None;
  }
  let meta_var = lang.extract_meta_var(&node.text())?;
  let replaced = match env.get(&meta_var)? {
    MatchResult::Single(replaced) => replaced
      .root
      .doc
      .get_source()
      .get_range(replaced.range())
      .to_vec(),
    MatchResult::Multi(nodes) => {
      if nodes.is_empty() {
        vec![]
      } else {
        // NOTE: start_byte is not always index range of source's slice.
        // e.g. start_byte is still byte_offset in utf_16 (napi). start_byte
        // so we need to call source's get_range method
        let start = nodes[0].inner.start_byte() as usize;
        let end = nodes[nodes.len() - 1].inner.end_byte() as usize;
        nodes[0]
          .root
          .doc
          .get_source()
          .get_range(start..end)
          .to_vec()
      }
    }
  };
  Some(replaced)
}

impl<'a, D: Doc> Replacer<D> for Node<'a, D> {
  fn generate_replacement(&self, _env: &MetaVarEnv<D>, _lang: D::Lang) -> Underlying<D::Source> {
    let range = self.range();
    self.root.doc.get_source().get_range(range).to_vec()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};
  use std::collections::HashMap;

  fn test_str_replace(replacer: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert(var.to_string(), root.root());
    }
    let replaced = replacer.generate_replacement(&env, Tsx);
    let replaced = String::from_utf8_lossy(&replaced);
    assert_eq!(
      replaced,
      expected,
      "wrong replacement {replaced} {expected} {:?}",
      HashMap::from(env)
    );
  }

  #[test]
  fn test_no_env() {
    test_str_replace("let a = 123", &[], "let a = 123");
    test_str_replace(
      "console.log('hello world'); let b = 123;",
      &[],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_single_env() {
    test_str_replace("let a = $A", &[("A", "123")], "let a = 123");
    test_str_replace(
      "console.log($HW); let b = 123;",
      &[("HW", "'hello world'")],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_multiple_env() {
    test_str_replace("let $V = $A", &[("A", "123"), ("V", "a")], "let a = 123");
    test_str_replace(
      "console.log($HW); let $B = 123;",
      &[("HW", "'hello world'"), ("B", "b")],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_multiple_occurrences() {
    test_str_replace("let $A = $A", &[("A", "a")], "let a = a");
    test_str_replace("var $A = () => $A", &[("A", "a")], "var a = () => a");
    test_str_replace(
      "const $A = () => { console.log($B); $A(); };",
      &[("B", "'hello world'"), ("A", "a")],
      "const a = () => { console.log('hello world'); a(); };",
    );
  }

  fn test_ellipsis_replace(replacer: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert_multi(var.to_string(), root.root().children().collect());
    }
    let replaced = replacer.generate_replacement(&env, Tsx);
    let replaced = String::from_utf8_lossy(&replaced);
    assert_eq!(
      replaced,
      expected,
      "wrong replacement {replaced} {expected} {:?}",
      HashMap::from(env)
    );
  }

  #[test]
  fn test_ellipsis_meta_var() {
    test_ellipsis_replace(
      "let a = () => { $$$B }",
      &[("B", "alert('works!')")],
      "let a = () => { alert('works!') }",
    );
    test_ellipsis_replace(
      "let a = () => { $$$B }",
      &[("B", "alert('works!');console.log(123)")],
      "let a = () => { alert('works!');console.log(123) }",
    );
  }

  #[test]
  fn test_multi_ellipsis() {
    test_ellipsis_replace(
      "import {$$$A, B, $$$C} from 'a'",
      &[("A", "A"), ("C", "C")],
      "import {A, B, C} from 'a'",
    );
  }

  #[test]
  fn test_replace_in_string() {
    test_str_replace("'$A'", &[("A", "123")], "'123'");
  }

  fn test_template_replace(template: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert(var.to_string(), root.root());
    }
    let ret = replace_meta_var_in_string(template, &env, &Tsx);
    assert_eq!(expected, ret);
  }

  #[test]
  fn test_template() {
    test_template_replace("Hello $A", &[("A", "World")], "Hello World");
    test_template_replace("$B $A", &[("A", "World"), ("B", "Hello")], "Hello World");
  }

  #[test]
  fn test_nested_matching_replace() {
    // TODO
  }
}
