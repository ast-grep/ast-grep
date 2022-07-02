use crate::meta_var::{extract_meta_var, MatchResult, MetaVarEnv};
use crate::ts_parser::Edit;
use crate::{Node, Root};
use std::collections::VecDeque;

pub trait Replacer {
    fn generate_replacement(&self, env: &MetaVarEnv) -> String;
}

impl<S: AsRef<str>> Replacer for S {
    fn generate_replacement(&self, env: &MetaVarEnv) -> String {
        let root = Root::new(self.as_ref());
        let mut stack = VecDeque::new();
        stack.push_back(root.root());
        let mut edits = vec![];
        while let Some(node) = stack.pop_front() {
            stack.extend(node.children());
            if let Some(text) = get_meta_var_replacement(&node, env) {
                let position = node.inner.start_byte();
                let length = node.inner.end_byte() - position;
                edits.push(Edit {
                    position,
                    deleted_length: length,
                    inserted_text: text,
                });
            }
        }
        let mut ret = String::new();
        let mut start = 0;
        for edit in edits {
            ret.push_str(&root.source[start..edit.position]);
            ret.extend(edit.inserted_text.chars());
            start = edit.position + edit.deleted_length;
        }
        ret
    }
}

fn get_meta_var_replacement(node: &Node, env: &MetaVarEnv) -> Option<String> {
    if !node.is_leaf() {
        return None;
    }
    let meta_var = extract_meta_var(node.text())?;
    let replaced = match env.get(&meta_var)? {
        MatchResult::Single(replaced) => replaced.text().to_string(),
        MatchResult::Multi(nodes) => nodes.iter().flat_map(|n| n.text().chars()).collect(),
    };
    Some(replaced)
}

impl<'a> Replacer for Node<'a> {
    fn generate_replacement(&self, _: &MetaVarEnv) -> String {
        self.text().to_string()
    }
}
