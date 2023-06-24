use crate::matcher::NodeMatch;
use crate::meta_var::{is_valid_meta_var_char, MetaVariableID};
use crate::source::{Content, Edit as E};
use crate::Pattern;
use crate::{Doc, Node, Root};

type Edit<D> = E<<D as Doc>::Source>;
type Underlying<S> = Vec<<S as Content>::Underlying>;

mod indent;
mod structural;
mod template;

pub use indent::IndentSensitive;
pub use template::{Fixer, FixerError};

/// Replace meta variable in the replacer string
pub trait Replacer<D: Doc> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source>;
}

impl<D: Doc> Replacer<D> for str
where
  D::Source: indent::IndentSensitive,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    template::gen_replacement(self, nm)
  }
}

impl<D: Doc> Replacer<D> for Root<D> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    structural::gen_replacement(self, nm)
  }
}

impl<D: Doc> Replacer<D> for Pattern<D> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    structural::gen_replacement(&self.root, nm)
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

impl<'a, D: Doc> Replacer<D> for Node<'a, D> {
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

fn split_first_meta_var<'a>(
  mut src: &'a str,
  meta_char: char,
  transform: &[MetaVariableID],
) -> Option<(MetaVarExtract, &'a str)> {
  debug_assert!(src.starts_with(meta_char));
  let mut i = 0;
  let (trimmed, is_multi) = loop {
    i += 1;
    src = &src[meta_char.len_utf8()..];
    if i == 3 {
      break (src, true);
    }
    if !src.starts_with(meta_char) {
      break (src, false);
    }
  };
  // no Anonymous meta var allowed, so _ is not allowed
  let i = trimmed
    .find(|c: char| !is_valid_meta_var_char(c))
    .unwrap_or(trimmed.len());
  // no name found
  if i == 0 {
    return None;
  }
  let name = trimmed[..i].to_string();
  let var = if is_multi {
    MetaVarExtract::Multiple(name)
  } else if transform.contains(&name) {
    MetaVarExtract::Transformed(name)
  } else {
    MetaVarExtract::Single(name)
  };
  Some((var, &trimmed[i..]))
}
