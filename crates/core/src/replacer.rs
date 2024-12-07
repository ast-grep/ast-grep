use crate::matcher::{Matcher, NodeMatch};
use crate::meta_var::{is_valid_meta_var_char, MetaVariableID};
use crate::source::Edit as E;
use crate::{Doc, Node, Root};
use std::ops::Range;

pub(crate) use indent::formatted_slice;

type Edit<D> = E<<D as Doc>::Source>;
type Underlying<S> = Vec<<S as Content>::Underlying>;

mod indent;
mod structural;
mod template;

pub use crate::source::Content;
pub use template::{TemplateFix, TemplateFixError};

/// Replace meta variable in the replacer string
pub trait Replacer<D: Doc> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source>;
  fn get_replaced_range(&self, nm: &NodeMatch<D>, matcher: impl Matcher<D::Lang>) -> Range<usize> {
    let range = nm.range();
    if let Some(len) = matcher.get_match_len(nm.get_node().clone()) {
      range.start..range.start + len
    } else {
      range
    }
  }
}

impl<D: Doc> Replacer<D> for str {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    template::gen_replacement(self, nm)
  }
}

impl<D: Doc> Replacer<D> for Root<D> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    structural::gen_replacement(self, nm)
  }
}

impl<D, T> Replacer<D> for &T
where
  D: Doc,
  T: Replacer<D> + ?Sized,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    (**self).generate_replacement(nm)
  }
}

impl<D: Doc> Replacer<D> for Node<'_, D> {
  fn generate_replacement(&self, _nm: &NodeMatch<D>) -> Underlying<D::Source> {
    let range = self.range();
    self.root.doc.get_source().get_range(range).to_vec()
  }
}

enum MetaVarExtract {
  /// $A for captured meta var
  Single(MetaVariableID),
  /// $$$A for captured ellipsis
  Multiple(MetaVariableID),
  Transformed(MetaVariableID),
}

impl MetaVarExtract {
  fn used_var(&self) -> &str {
    match self {
      MetaVarExtract::Single(s) => s,
      MetaVarExtract::Multiple(s) => s,
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
    MetaVarExtract::Multiple(name)
  } else if transform.contains(&name) {
    MetaVarExtract::Transformed(name)
  } else {
    MetaVarExtract::Single(name)
  };
  Some((var, skipped + i))
}
