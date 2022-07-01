use crate::pattern::Pattern;
use crate::Node;
use std::collections::HashMap;
use crate::matcher::does_node_match_exactly;

pub type MetaVariableID = String;

// a dictionary for metavariable instantiation
// const a = 123 matched with const a = $A will produce env: $A => 123
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

    pub fn get(&self, var: &MetaVariable) -> Option<MatchResult<'tree>> {
        // TODO: optimize this copied/cloned behavior
        match var {
            MetaVariable::Named(n) => self.single_matched.get(n).copied().map(MatchResult::Single),
            MetaVariable::NamedEllipsis(n) => {
                self.multi_matched.get(n).cloned().map(MatchResult::Multi)
            }
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

pub enum MatchResult<'tree> {
    // $A for captured meta var
    Single(Node<'tree>),
    // $$$A for captured ellipsis
    Multi(Vec<Node<'tree>>),
}

pub enum MetaVariable {
    // $A for captured meta var
    Named(MetaVariableID),
    // $_ for non-captured meta var
    Anonymous,
    // $$$ for non-captured ellipsis
    Ellipsis,
    // $$$A for captured ellipsis
    NamedEllipsis(MetaVariableID),
}

pub enum MetaVarMatcher {
    // A regex to filter matched metavar based on its textual content.
    Regex(&'static str),
    // A pattern to filter matched metavar based on its AST tree shape.
    Pattern(Pattern),
}

impl MetaVarMatcher {
    pub fn matches(&self, candidate: Node) -> bool {
        use MetaVarMatcher::*;
        match self {
            Regex(s) => todo!(),
            Pattern(p) => p.match_node(candidate).is_some(),
        }
    }
}

pub fn extract_meta_var(s: &str) -> Option<MetaVariable> {
    use MetaVariable::*;
    if s == "$$$" {
        return Some(Ellipsis);
    }
    if let Some(trimmed) = s.strip_prefix("$$$") {
        if !trimmed.chars().all(is_valid_meta_var_char) {
            return None;
        }
        if trimmed.starts_with('_') {
            return Some(Ellipsis);
        } else {
            return Some(NamedEllipsis(trimmed.to_owned()));
        }
    }
    if !s.starts_with('$') {
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
