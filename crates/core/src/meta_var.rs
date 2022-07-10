use crate::matcher::does_node_match_exactly;
use crate::pattern::Pattern;
use crate::Node;
use std::collections::HashMap;

pub type MetaVariableID = String;

/// a dictionary that stores metavariable instantiation
/// const a = 123 matched with const a = $A will produce env: $A => 123
#[derive(Default)]
pub struct MetaVarEnv<'tree> {
    var_matchers: HashMap<MetaVariableID, MetaVarMatcher>,
    single_matched: HashMap<MetaVariableID, Node<'tree>>,
    multi_matched: HashMap<MetaVariableID, Vec<Node<'tree>>>,
}

impl<'tree> MetaVarEnv<'tree> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, id: MetaVariableID, ret: Node<'tree>) -> Option<&mut Self> {
        if !self.match_variable(&id, ret) {
            return None;
        }
        self.single_matched.insert(id, ret);
        Some(self)
    }

    pub fn insert_multi(&mut self, id: MetaVariableID, ret: Vec<Node<'tree>>) -> Option<&mut Self> {
        self.multi_matched.insert(id, ret);
        Some(self)
    }

    pub fn get(&self, var: &MetaVariable) -> Option<MatchResult<'_, 'tree>> {
        match var {
            MetaVariable::Named(n) => self.single_matched.get(n).map(MatchResult::Single),
            MetaVariable::NamedEllipsis(n) => self.multi_matched.get(n).map(MatchResult::Multi),
            _ => None,
        }
    }
}

impl<'tree> From<MetaVarEnv<'tree>> for HashMap<String, String> {
    fn from(env: MetaVarEnv<'tree>) -> Self {
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

impl<'tree> MetaVarEnv<'tree> {
    fn match_variable(&self, id: &MetaVariableID, candidate: Node) -> bool {
        if let Some(m) = self.var_matchers.get(id) {
            if !m.matches(candidate) {
                return false;
            }
        }
        if let Some(m) = self.single_matched.get(id) {
            return does_node_match_exactly(m, candidate);
        }
        true
    }
}

pub enum MatchResult<'a, 'tree> {
    /// $A for captured meta var
    Single(&'a Node<'tree>),
    /// $$$A for captured ellipsis
    Multi(&'a Vec<Node<'tree>>),
}

#[derive(Debug, PartialEq)]
pub enum MetaVariable {
    /// $A for captured meta var
    Named(MetaVariableID),
    /// $_ for non-captured meta var
    Anonymous,
    /// $$$ for non-captured ellipsis
    Ellipsis,
    /// $$$A for captured ellipsis
    NamedEllipsis(MetaVariableID),
}

pub enum MetaVarMatcher {
    /// A regex to filter matched metavar based on its textual content.
    Regex(&'static str),
    /// A pattern to filter matched metavar based on its AST tree shape.
    Pattern(Pattern),
}

impl MetaVarMatcher {
    pub fn matches(&self, candidate: Node) -> bool {
        use crate::rule::Matcher;
        use MetaVarMatcher::*;
        let mut env = MetaVarEnv::new();
        match self {
            Regex(_s) => todo!(),
            Pattern(p) => p.match_node(candidate, &mut env).is_some(),
        }
    }
}

pub(crate) fn extract_meta_var(s: &str, meta_char: char) -> Option<MetaVariable> {
    use MetaVariable::*;
    let ellipsis: String = std::iter::repeat(meta_char).take(3).collect();
    if s == ellipsis {
        return Some(Ellipsis);
    }
    if let Some(trimmed) = s.strip_prefix(&ellipsis) {
        if !trimmed.chars().all(is_valid_meta_var_char) {
            return None;
        }
        if trimmed.starts_with('_') {
            return Some(Ellipsis);
        } else {
            return Some(NamedEllipsis(trimmed.to_owned()));
        }
    }
    if !s.starts_with(meta_char) {
        return None;
    }
    let trimmed = &s[1..];
    // $A or $_
    if !trimmed.chars().all(is_valid_meta_var_char) {
        return None;
    }
    if trimmed.starts_with('_') {
        Some(Anonymous)
    } else {
        Some(Named(trimmed.to_owned()))
    }
}

fn is_valid_meta_var_char(c: char) -> bool {
    matches!(c, 'A'..='Z' | '_')
}

#[cfg(test)]
mod test {
    use super::*;

    fn extract_var(s: &str) -> Option<MetaVariable> {
        extract_meta_var(s, '$')
    }
    #[test]
    fn test_match_var() {
        use MetaVariable::*;
        assert_eq!(extract_var("$$$"), Some(Ellipsis));
        assert_eq!(extract_var("$ABC"), Some(Named("ABC".into())));
        assert_eq!(
            extract_var("$$$ABC"),
            Some(NamedEllipsis("ABC".into()))
        );
        assert_eq!(extract_var("$_"), Some(Anonymous));
        assert_eq!(extract_var("abc"), None);
        assert_eq!(extract_var("$abc"), None);
    }
}
