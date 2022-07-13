use tree_sitter::{InputEdit, Parser, Point};
pub use tree_sitter::{Language, Tree};

pub fn parse(source_code: &str, old_tree: Option<&Tree>, ts_lang: Language) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(ts_lang).unwrap();
    parser.parse(source_code, old_tree).unwrap()
}

// https://github.com/tree-sitter/tree-sitter/blob/e4e5ffe517ca2c668689b24cb17c51b8c6db0790/cli/src/parse.rs
#[derive(Debug)]
pub struct Edit {
    pub position: usize,
    pub deleted_length: usize,
    pub inserted_text: String,
}

fn position_for_offset(input: &Vec<u8>, offset: usize) -> Point {
    let mut result = Point { row: 0, column: 0 };
    for c in &input[0..offset] {
        if *c as char == '\n' {
            result.row += 1;
            result.column = 0;
        } else {
            result.column += 1;
        }
    }
    result
}

pub fn perform_edit(tree: &mut Tree, input: &mut Vec<u8>, edit: &Edit) -> InputEdit {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let start_position = position_for_offset(input, start_byte);
    let old_end_position = position_for_offset(input, old_end_byte);
    input.splice(start_byte..old_end_byte, edit.inserted_text.bytes());
    let new_end_position = position_for_offset(input, new_end_byte);
    let edit = InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    };
    tree.edit(&edit);
    edit
}

#[cfg(test)]
mod test {
    use super::{parse as parse_lang, *};
    use crate::language::{Language, Tsx};

    fn parse(src: &str) -> Tree {
        parse_lang(src, None, Tsx.get_ts_language())
    }

    #[test]
    fn test_tree_sitter() {
        let tree = parse("var a = 1234");
        let root_node = tree.root_node();
        assert_eq!(root_node.kind(), "program");
        assert_eq!(root_node.start_position().column, 0);
        assert_eq!(root_node.end_position().column, 12);
        assert_eq!(root_node.to_sexp(), "(program (variable_declaration (variable_declarator name: (identifier) value: (number))))");
    }

    #[test]
    fn test_object_literal() {
        let tree = parse("{a: $X}");
        let root_node = tree.root_node();
        // wow this is not label. technically it is wrong but practically it is better LOL
        assert_eq!(root_node.to_sexp(), "(program (expression_statement (object (pair key: (property_identifier) value: (identifier)))))");
    }

    #[test]
    fn test_edit() {
        let mut src = "a + b".to_string();
        let mut tree = parse(&src);
        let edit = perform_edit(
            &mut tree,
            unsafe { src.as_mut_vec() },
            &Edit {
                position: 1,
                deleted_length: 0,
                inserted_text: " * b".into(),
            },
        );
        tree.edit(&edit);
        let tree2 = parse_lang(&src, Some(&tree), Tsx.get_ts_language());
        assert_eq!(tree.root_node().to_sexp(), "(program (expression_statement (binary_expression left: (identifier) right: (identifier))))");
        assert_eq!(tree2.root_node().to_sexp(), "(program (expression_statement (binary_expression left: (binary_expression left: (binary_expression left: (identifier) right: (identifier)) right: (identifier)) right: (identifier))))");
    }
}
