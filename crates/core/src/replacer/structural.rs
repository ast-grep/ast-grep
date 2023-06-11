use super::{Edit, Underlying};
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::MetaVarEnv;
use crate::source::Content;
use crate::{Doc, Node, Root};

pub fn gen_replacement<D: Doc>(root: &Root<D>, nm: &NodeMatch<D>) -> Underlying<D::Source> {
  let edits = collect_edits(root, nm.get_env(), nm.lang());
  merge_edits_to_vec(edits, root)
}

fn collect_edits<D: Doc>(root: &Root<D>, env: &MetaVarEnv<D>, lang: &D::Lang) -> Vec<Edit<D>> {
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
  let replaced = env.get_var_bytes(&meta_var)?;
  Some(replaced.to_vec())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};
  use crate::meta_var::MetaVarEnv;
  use crate::{replacer::Replacer, Root};
  use std::collections::HashMap;

  fn test_pattern_replace(replacer: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert(var.to_string(), root.root());
    }
    let dummy = Tsx.ast_grep("dummy");
    let node_match = NodeMatch::new(dummy.root(), env.clone());
    let replacer = Root::new(replacer, Tsx);
    let replaced = replacer.generate_replacement(&node_match);
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
    test_pattern_replace("let a = 123", &[], "let a = 123");
    test_pattern_replace(
      "console.log('hello world'); let b = 123;",
      &[],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_single_env() {
    test_pattern_replace("let a = $A", &[("A", "123")], "let a = 123");
    test_pattern_replace(
      "console.log($HW); let b = 123;",
      &[("HW", "'hello world'")],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_multiple_env() {
    test_pattern_replace("let $V = $A", &[("A", "123"), ("V", "a")], "let a = 123");
    test_pattern_replace(
      "console.log($HW); let $B = 123;",
      &[("HW", "'hello world'"), ("B", "b")],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_multiple_occurrences() {
    test_pattern_replace("let $A = $A", &[("A", "a")], "let a = a");
    test_pattern_replace("var $A = () => $A", &[("A", "a")], "var a = () => a");
    test_pattern_replace(
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
    let dummy = Tsx.ast_grep("dummy");
    let node_match = NodeMatch::new(dummy.root(), env.clone());
    let replacer = Root::new(replacer, Tsx);
    let replaced = replacer.generate_replacement(&node_match);
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
    test_pattern_replace("'$A'", &[("A", "123")], "'123'");
  }

  #[test]
  fn test_nested_matching_replace() {
    // TODO
  }
}
