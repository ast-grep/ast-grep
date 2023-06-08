use crate::matcher::NodeMatch;
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
