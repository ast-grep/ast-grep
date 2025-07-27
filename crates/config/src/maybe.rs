use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{de, ser, Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, PartialEq, Eq, Debug, Copy, Default)]
pub enum Maybe<T> {
  #[default]
  Absent,
  Present(T),
}

impl<T> Maybe<T> {
  pub fn is_present(&self) -> bool {
    matches!(self, Maybe::Present(_))
  }
  pub fn is_absent(&self) -> bool {
    matches!(self, Maybe::Absent)
  }
  pub fn unwrap(self) -> T {
    match self {
      Maybe::Absent => panic!("called `Maybe::unwrap()` on an `Absent` value"),
      Maybe::Present(t) => t,
    }
  }
}

impl<T> From<Maybe<T>> for Option<T> {
  fn from(maybe: Maybe<T>) -> Self {
    match maybe {
      Maybe::Present(v) => Some(v),
      Maybe::Absent => None,
    }
  }
}

impl<T> From<Option<T>> for Maybe<T> {
  fn from(opt: Option<T>) -> Maybe<T> {
    match opt {
      Some(v) => Maybe::Present(v),
      None => Maybe::Absent,
    }
  }
}

const ERROR_STR: &str = r#"Maybe fields need to be annotated with:
  #[serde(default, skip_serializing_if = "Maybe::is_absent")]"#;

impl<T: Serialize> Serialize for Maybe<T> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self {
      Maybe::Absent => Err(ser::Error::custom(ERROR_STR)),
      Maybe::Present(t) => T::serialize(t, serializer),
    }
  }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Maybe<T> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    match Option::deserialize(deserializer)? {
      Some(t) => Ok(Maybe::Present(t)),
      None => Err(de::Error::custom("Maybe field cannot be null.")),
    }
  }
}

impl<T: JsonSchema> JsonSchema for Maybe<T> {
  fn schema_name() -> Cow<'static, str> {
    Cow::Owned(format!("Maybe_{}", T::schema_name()))
  }
  fn schema_id() -> Cow<'static, str> {
    Cow::Owned(format!("Maybe<{}>", T::schema_id()))
  }
  fn json_schema(gen: &mut SchemaGenerator) -> Schema {
    gen.subschema_for::<T>()
  }

  fn inline_schema() -> bool {
    true
  }

  fn _schemars_private_non_optional_json_schema(gen: &mut SchemaGenerator) -> Schema {
    T::_schemars_private_non_optional_json_schema(gen)
  }

  fn _schemars_private_is_option() -> bool {
    true
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;

  #[derive(Serialize, Deserialize, Debug)]
  struct Correct {
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    a: Maybe<i32>,
  }
  #[derive(Serialize, Deserialize, Debug)]
  struct Wrong {
    #[serde(skip_serializing_if = "Maybe::is_absent")]
    a: Maybe<i32>,
  }

  #[test]
  fn test_de_correct_ok() {
    let correct: Correct = from_str("a: 123").expect("should ok");
    assert!(matches!(correct.a, Maybe::Present(123)));
    let correct: Correct = from_str("").expect("should ok");
    assert!(matches!(correct.a, Maybe::Absent));
  }
  #[test]
  fn test_de_correct_err() {
    let ret: Result<Correct, _> = from_str("a:");
    assert!(ret.is_err());
    let err = ret.unwrap_err().to_string();
    assert!(err.contains("cannot be null"));
  }
  #[test]
  fn test_de_wrong_err() {
    let wrong: Wrong = from_str("a: 123").expect("should ok");
    assert!(matches!(wrong.a, Maybe::Present(123)));
    let wrong: Result<Wrong, _> = from_str("a:");
    assert!(wrong.is_err());
    let wrong: Result<Wrong, _> = from_str("");
    assert!(wrong.is_err());
  }

  #[test]
  #[should_panic]
  fn test_unwrap_absent() {
    let nothing: Maybe<()> = Maybe::Absent;
    nothing.unwrap();
  }

  #[test]
  fn test_from_optio() {
    let mut maybe = Maybe::from(None);
    assert!(maybe.is_absent());
    maybe = Maybe::from(Some(123));
    assert!(maybe.is_present());
  }
}
