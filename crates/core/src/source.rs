use crate::language::Language;
use std::ops::Deref;

pub trait Doc: Clone {
  type Repr: Content;
  type Lang: Language;
  fn get_lang(&self) -> &Self::Lang;
  fn get_source(&self) -> &str;
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
  fn get_source(&self) -> &str {
    &self.src
  }
}

// Content is thread safe and owns the data
pub trait Content: ToString + Deref<Target = str> + Send + Sync + 'static {
  fn as_mut_vec(&mut self) -> &mut Vec<u8>;
}

pub enum Source {
  Plain(String),
  Customized(Box<dyn Content>),
}

use Source::*;

impl From<&str> for Source {
  fn from(s: &str) -> Self {
    Plain(s.into())
  }
}

impl Clone for Source {
  fn clone(&self) -> Self {
    match self {
      Plain(s) => Plain(s.clone()),
      Customized(_) => todo!(),
    }
  }
}

impl Deref for Source {
  type Target = str;
  fn deref(&self) -> &Self::Target {
    match self {
      Plain(s) => s.deref(),
      Customized(c) => c.deref(),
    }
  }
}

impl ToString for Source {
  fn to_string(&self) -> String {
    match self {
      Self::Plain(s) => s.to_owned(),
      Self::Customized(c) => c.to_string(),
    }
  }
}
impl Content for String {
  fn as_mut_vec(&mut self) -> &mut Vec<u8> {
    unsafe { self.as_mut_vec() }
  }
}

impl Content for Source {
  fn as_mut_vec(&mut self) -> &mut Vec<u8> {
    match self {
      Plain(s) => unsafe { s.as_mut_vec() },
      Customized(c) => c.as_mut_vec(),
    }
  }
}
