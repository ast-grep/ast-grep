use super::{Edit, Underlying};
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::{MatchResult, MetaVarEnv};
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
