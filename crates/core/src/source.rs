use crate::language::Language;
use std::ops::Deref;

pub trait Doc: Clone {
  type Repr: Content;
  type Lang: Language;
  fn get_lang(&self) -> &Self::Lang;
  fn get_source(&self) -> &Self::Repr;
  /// # Safety
  /// TODO
  unsafe fn as_mut(&mut self) -> &mut Vec<u8>;
}

#[derive(Clone)]
pub struct StrDoc<L: Language> {
  pub src: String,
  pub lang: L,
}
impl<L: Language> StrDoc<L> {
  pub fn new(src: &str, lang: L) -> Self {
    Self {
      src: src.into(),
      lang,
    }
  }
}

impl<L: Language> Doc for StrDoc<L> {
  type Repr = String;
  type Lang = L;
  fn get_lang(&self) -> &Self::Lang {
    &self.lang
  }
  fn get_source(&self) -> &Self::Repr {
    &self.src
  }
  unsafe fn as_mut(&mut self) -> &mut Vec<u8> {
    self.src.as_mut_vec()
  }
}

// Content is thread safe and owns the data
pub trait Content: ToString + Deref<Target = str> {}

impl Content for String {}
