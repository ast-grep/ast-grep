use tree_sitter::{Parser, Language};
pub use tree_sitter::Tree;

extern "C" {
    fn tree_sitter_tsx() -> Language;
}

pub fn parse(source_code: &str) -> Tree {
    let mut parser = Parser::new();
    let language = unsafe { tree_sitter_tsx() };
    parser.set_language(language).unwrap();
    parser.parse(source_code, None).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;

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
}
