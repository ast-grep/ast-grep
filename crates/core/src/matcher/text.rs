use super::Matcher;
use crate::meta_var::MetaVarEnv;
use crate::{Doc, Language, Node};

use bit_set::BitSet;
use regex::{Error as RegexError, Regex};
use thiserror::Error;

use std::borrow::Cow;
use std::marker::PhantomData;

#[derive(Debug, Error)]
pub enum RegexMatcherError {
  #[error("Parsing text matcher fails.")]
  Regex(#[from] RegexError),
}

#[derive(Clone)]
pub struct RegexMatcher<L: Language> {
  regex: Regex,
  lang: PhantomData<L>,
}

impl<L: Language> RegexMatcher<L> {
  pub fn try_new(text: &str) -> Result<Self, RegexMatcherError> {
    Ok(RegexMatcher {
      regex: Regex::new(text)?,
      lang: PhantomData,
    })
  }
}

impl<L: Language> Matcher<L> for RegexMatcher<L> {
  fn match_node_with_env<'tree, D: Doc<Lang = L>>(
    &self,
    node: Node<'tree, D>,
    _env: &mut Cow<MetaVarEnv<'tree, D>>,
  ) -> Option<Node<'tree, D>> {
    self.regex.is_match(&node.text()).then_some(node)
  }

  fn potential_kinds(&self) -> Option<BitSet> {
    None
  }
}
