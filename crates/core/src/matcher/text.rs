use super::Matcher;
use crate::meta_var::MetaVarEnv;
use crate::Language;
use crate::Node;

use bit_set::BitSet;
use regex::{Error as RegexError, Regex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegexMatcherError {
  #[error("Parsing text matcher fails.")]
  Regex(#[from] RegexError),
}

#[derive(Clone)]
pub struct RegexMatcher {
  regex: Regex,
}

impl RegexMatcher {
  pub fn try_new(text: &str) -> Result<Self, RegexMatcherError> {
    Ok(RegexMatcher {
      regex: Regex::new(text)?,
    })
  }
}

impl<L: Language> Matcher<L> for RegexMatcher {
  fn match_node_with_env<'tree>(
    &self,
    node: Node<'tree, L>,
    _env: &mut MetaVarEnv<'tree, L>,
  ) -> Option<Node<'tree, L>> {
    self.regex.is_match(&node.text()).then_some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }
}
