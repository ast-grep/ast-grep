use crate::matcher::Matcher;
use crate::meta_var::{MetaVariable, MetaVariableID, Underlying, is_valid_meta_var_char};
use crate::{Doc, Node, NodeMatch, Root};
use std::ops::Range;

pub(crate) use indent::formatted_slice;

use crate::source::Edit as E;
type Edit<D> = E<<D as Doc>::Source>;

mod indent;
mod structural;
mod template;

pub use crate::source::Content;
pub use template::{TemplateFix, TemplateFixError};

/// Replace meta variable in the replacer string
pub trait Replacer<D: Doc> {
  fn generate_replacement(&self, nm: &NodeMatch<'_, D>) -> Underlying<D>;
  fn get_replaced_range(&self, nm: &NodeMatch<'_, D>, matcher: impl Matcher) -> Range<usize> {
    let range = nm.range();
    if let Some(len) = matcher.get_match_len(nm.get_node().clone()) {
      range.start..range.start + len
    } else {
      range
    }
  }
}

impl<D: Doc> Replacer<D> for str {
  fn generate_replacement(&self, nm: &NodeMatch<'_, D>) -> Underlying<D> {
    template::gen_replacement(self, nm)
  }
}

impl<D: Doc> Replacer<D> for Root<D> {
  fn generate_replacement(&self, nm: &NodeMatch<'_, D>) -> Underlying<D> {
    structural::gen_replacement(self, nm)
  }
}

impl<D, T> Replacer<D> for &T
where
  D: Doc,
  T: Replacer<D> + ?Sized,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D> {
    (**self).generate_replacement(nm)
  }
}

impl<D: Doc> Replacer<D> for Node<'_, D> {
  fn generate_replacement(&self, _nm: &NodeMatch<'_, D>) -> Underlying<D> {
    let range = self.range();
    self.root.doc.get_source().get_range(range).to_vec()
  }
}

enum MetaVarExtract {
  Node(MetaVariable),
  Transformed(MetaVariableID),
}

impl MetaVarExtract {
  fn used_var(&self) -> &str {
    match self {
      MetaVarExtract::Node(MetaVariable::Capture(s, _))
      | MetaVarExtract::Node(MetaVariable::MultiCapture(s)) => s,
      MetaVarExtract::Node(MetaVariable::Dropped(_) | MetaVariable::Multiple) => {
        unreachable!("template variables must be captured")
      }
      MetaVarExtract::Transformed(s) => s,
    }
  }
}

fn split_first_meta_var(
  src: &str,
  meta_char: char,
  transform: &[MetaVariableID],
) -> Option<(MetaVarExtract, usize)> {
  debug_assert!(src.starts_with(meta_char));
  let mut i = 0;
  let mut skipped = 0;
  let is_multi = loop {
    i += 1;
    skipped += meta_char.len_utf8();
    if i == 3 {
      break true;
    }
    if !src[skipped..].starts_with(meta_char) {
      break false;
    }
  };
  let is_named = i == 1;
  // no Anonymous meta var allowed, so _ is not allowed
  let i = src[skipped..]
    .find(|c: char| !is_valid_meta_var_char(c))
    .unwrap_or(src.len() - skipped);
  // no name found
  if i == 0 {
    return None;
  }
  let name = src[skipped..skipped + i].to_string();
  let var = if is_multi {
    MetaVarExtract::Node(MetaVariable::MultiCapture(name))
  } else if transform.contains(&name) {
    MetaVarExtract::Transformed(name)
  } else {
    MetaVarExtract::Node(MetaVariable::Capture(name, is_named))
  };
  Some((var, skipped + i))
}
