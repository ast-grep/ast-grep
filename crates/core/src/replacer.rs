use crate::language::Language;
use crate::meta_var::{MatchResult, MetaVarEnv};
use crate::ts_parser::Edit;
use crate::{Node, Root};
use crate::Pattern;

/// Replace meta variable in the replacer string
pub trait Replacer<L: Language> {
    fn generate_replacement(&self, env: &MetaVarEnv<L>, lang: L) -> String;
}

impl<S: AsRef<str>, L: Language> Replacer<L> for S {
    fn generate_replacement(&self, env: &MetaVarEnv<L>, lang: L) -> String {
        let root = Root::new(self.as_ref(), lang);
        let edits = collect_edits(&root, env, lang);
        merge_edits_to_string(edits, &root)
    }
}

impl<L: Language> Replacer<L> for Pattern<L> {
    fn generate_replacement(&self, env: &MetaVarEnv<L>, lang: L) -> String {
        let edits = collect_edits(&self.root, env, lang);
        merge_edits_to_string(edits, &self.root)
    }
}


fn collect_edits<L: Language>(
    root: &Root<L>,
    env: &MetaVarEnv<L>,
    lang: L,
) -> Vec<Edit> {
    let mut node = root.root();
    let root_id = node.inner.id();
    let mut edits = vec![];

    // this is a preorder DFS that stops traversal when the node matches
    loop {
        if let Some(text) = get_meta_var_replacement(&node, env, lang) {
            let position = node.inner.start_byte();
            let length = node.inner.end_byte() - position;
            edits.push(Edit {
                position,
                deleted_length: length,
                inserted_text: text,
            });
        } else if let Some(first_child) = node.nth_child(0) {
            node = first_child;
            continue;
        }
        loop {
            // come back to the root node, terminating dfs
            if node.inner.id() == root_id {
                // add the missing one
                edits.push(Edit {
                    position: root.source.len(),
                    deleted_length: 0,
                    inserted_text: String::new(),
                });
                return edits
            }
            if let Some(sibling) = node.next() {
                node = sibling;
                break;
            }
            node = node.parent().unwrap();
        }
    }
}

fn merge_edits_to_string<L: Language>(edits: Vec<Edit>, root: &Root<L>) -> String {
    let mut ret = String::new();
    let mut start = 0;
    for edit in edits {
        ret.push_str(&root.source[start..edit.position]);
        ret.extend(edit.inserted_text.chars());
        start = edit.position + edit.deleted_length;
    }
    ret
}

fn get_meta_var_replacement<L: Language>(
    node: &Node<L>,
    env: &MetaVarEnv<L>,
    lang: L,
) -> Option<String> {
    if !node.is_leaf() {
        return None;
    }
    let meta_var = lang.extract_meta_var(node.text())?;
    let replaced = match env.get(&meta_var)? {
        MatchResult::Single(replaced) => replaced.text().to_string(),
        MatchResult::Multi(nodes) => {
            if nodes.is_empty() {
                String::new()
            } else {
                let start = nodes[0].inner.start_byte();
                let end = nodes[nodes.len() - 1].inner.end_byte();
                nodes[0].root.source[start..end].to_string()
            }
        }
    };
    Some(replaced)
}

impl<'a, L: Language> Replacer<L> for Node<'a, L> {
    fn generate_replacement(&self, _: &MetaVarEnv<L>, _: L) -> String {
        self.text().to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::language::{Language, Tsx};
    use std::collections::HashMap;

    fn test_str_replace(replacer: &str, vars: &[(&str, &str)], expected: &str) {
        let mut env = MetaVarEnv::new();
        let roots: Vec<_> = vars.iter().map(|(v, p)| (v, Tsx.new(p).inner)).collect();
        for (var, root) in &roots {
            env.insert(var.to_string(), root.root());
        }
        let replaced = replacer.generate_replacement(&env, Tsx);
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
        let roots: Vec<_> = vars.iter().map(|(v, p)| (v, Tsx.new(p).inner)).collect();
        for (var, root) in &roots {
            env.insert_multi(var.to_string(), root.root().children().collect());
        }
        let replaced = replacer.generate_replacement(&env, Tsx);
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
    fn test_nested_matching_replace() {
        // TODO
    }
}
