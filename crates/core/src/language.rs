use crate::matcher::PatternBuilder;
use crate::meta_var::{extract_meta_var, MetaVariable};
use crate::{Pattern, PatternError};
use std::borrow::Cow;
use std::path::Path;

/// Trait to abstract ts-language usage in ast-grep, which includes:
/// * which character is used for meta variable.
/// * if we need to use other char in meta var for parser at runtime
/// * pre process the Pattern code.
pub trait Language: Clone + 'static {
  /// normalize pattern code before matching
  /// e.g. remove expression_statement, or prefer parsing {} to object over block
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    Cow::Borrowed(query)
  }

  /// Configure meta variable special character
  /// By default $ is the metavar char, but in PHP it can be #
  #[inline]
  fn meta_var_char(&self) -> char {
    '$'
  }

  /// Some language does not accept $ as the leading char for identifiers.
  /// We need to change $ to other char at run-time to make parser happy, thus the name expando.
  /// By default this is the same as meta_var char so replacement is done at runtime.
  #[inline]
  fn expando_char(&self) -> char {
    self.meta_var_char()
  }

  /// extract MetaVariable from a given source string
  /// At runtime we need to use expand_char
  fn extract_meta_var(&self, source: &str) -> Option<MetaVariable> {
    extract_meta_var(source, self.expando_char())
  }
  /// Return the file language from path. Return None if the file type is not supported.
  fn from_path<P: AsRef<Path>>(_path: P) -> Option<Self> {
    // TODO: throw panic here if not implemented properly?
    None
  }

  fn kind_to_id(&self, kind: &str) -> u16;
  fn field_to_id(&self, field: &str) -> Option<u16>;
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError>;
}

#[cfg(test)]
pub use test::*;

#[cfg(test)]
mod test {
  use super::*;
  use crate::tree_sitter::{LanguageExt, StrDoc, TSLanguage};

  #[derive(Clone)]
  pub struct Tsx;
  impl Language for Tsx {
    fn kind_to_id(&self, kind: &str) -> u16 {
      let ts_lang: TSLanguage = tree_sitter_typescript::LANGUAGE_TSX.into();
      ts_lang.id_for_node_kind(kind, /* named */ true)
    }
    fn field_to_id(&self, field: &str) -> Option<u16> {
      self
        .get_ts_language()
        .field_id_for_name(field)
        .map(|f| f.get())
    }
    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
      builder.build(|src| StrDoc::try_new(src, self.clone()))
    }
  }
  impl LanguageExt for Tsx {
    fn get_ts_language(&self) -> TSLanguage {
      tree_sitter_typescript::LANGUAGE_TSX.into()
    }
  }
}
