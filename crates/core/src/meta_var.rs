use crate::matcher::does_node_match_exactly;
use crate::pattern::Pattern;
use crate::Language;
use crate::Node;
use std::collections::HashMap;

pub type MetaVariableID = String;

/// a dictionary that stores metavariable instantiation
/// const a = 123 matched with const a = $A will produce env: $A => 123
pub struct MetaVarEnv<'tree, L: Language> {
    var_matchers: HashMap<MetaVariableID, MetaVarMatcher<L>>,
    single_matched: HashMap<MetaVariableID, Node<'tree, L>>,
    multi_matched: HashMap<MetaVariableID, Vec<Node<'tree, L>>>,
}

impl<'tree, L: Language> MetaVarEnv<'tree, L> {
    pub fn new() -> Self {
        Self {
            var_matchers: HashMap::new(),
            single_matched: HashMap::new(),
            multi_matched: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: MetaVariableID, ret: Node<'tree, L>) -> Option<&mut Self> {
        if !self.match_variable(&id, ret) {
            return None;
        }
        self.single_matched.insert(id, ret);
        Some(self)
    }

    pub fn insert_multi(
        &mut self,
        id: MetaVariableID,
        ret: Vec<Node<'tree, L>>,
    ) -> Option<&mut Self> {
        self.multi_matched.insert(id, ret);
        Some(self)
    }

    pub fn get(&self, var: &MetaVariable) -> Option<MatchResult<'_, 'tree, L>> {
        match var {
            MetaVariable::Named(n) => self.single_matched.get(n).map(MatchResult::Single),
            MetaVariable::NamedEllipsis(n) => self.multi_matched.get(n).map(MatchResult::Multi),
            _ => None,
        }
    }
}

impl<'tree, L: Language> From<MetaVarEnv<'tree, L>> for HashMap<String, String> {
    fn from(env: MetaVarEnv<'tree, L>) -> Self {
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

impl<'tree, L: Language> MetaVarEnv<'tree, L> {
    fn match_variable(&self, id: &MetaVariableID, candidate: Node<L>) -> bool {
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

pub enum MatchResult<'a, 'tree, L: Language> {
    /// $A for captured meta var
    Single(&'a Node<'tree, L>),
    /// $$$A for captured ellipsis
    Multi(&'a Vec<Node<'tree, L>>),
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

pub enum MetaVarMatcher<L: Language> {
    /// A regex to filter matched metavar based on its textual content.
    Regex(&'static str),
    /// A pattern to filter matched metavar based on its AST tree shape.
    Pattern(Pattern<L>),
}

impl<L: Language> MetaVarMatcher<L> {
    pub fn matches(&self, candidate: Node<L>) -> bool {
        use crate::rule::Matcher;
        use MetaVarMatcher::*;
        let mut env = MetaVarEnv::new();
        match self {
            Regex(_s) => todo!(),
            Pattern(p) => p.match_node(candidate, &mut env).is_some(),
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
    let trimmed = &src[1..];
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
        assert_eq!(extract_var("$$$ABC"), Some(NamedEllipsis("ABC".into())));
        assert_eq!(extract_var("$_"), Some(Anonymous));
        assert_eq!(extract_var("abc"), None);
        assert_eq!(extract_var("$abc"), None);
    }
}
