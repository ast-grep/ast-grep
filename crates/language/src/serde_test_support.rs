//! Record the Serde variant index; JSON alone cannot detect index changes.
use serde::ser::Impossible;
use serde::{Serialize, Serializer};

type Recorded = (u32, &'static str);
type Compound = Impossible<Recorded, serde::de::value::Error>;
pub(super) struct IndexSerializer;

macro_rules! reject {
  ($($method:ident($($argument:ident: $ty:ty),*) -> $result:ty;)*) => { $(
    fn $method(self, $($argument: $ty),*) -> Result<$result, Self::Error> {
      unreachable!("SupportLang must serialize as a unit variant")
    }
  )* };
}

impl Serializer for IndexSerializer {
  type Ok = Recorded;
  type Error = serde::de::value::Error;
  type SerializeSeq = Compound;
  type SerializeTuple = Compound;
  type SerializeTupleStruct = Compound;
  type SerializeTupleVariant = Compound;
  type SerializeMap = Compound;
  type SerializeStruct = Compound;
  type SerializeStructVariant = Compound;

  fn serialize_unit_variant(
    self,
    _name: &'static str,
    index: u32,
    variant: &'static str,
  ) -> Result<Self::Ok, Self::Error> {
    Ok((index, variant))
  }

  reject! {
    serialize_bool(_v: bool) -> Self::Ok;
    serialize_i8(_v: i8) -> Self::Ok;
    serialize_i16(_v: i16) -> Self::Ok;
    serialize_i32(_v: i32) -> Self::Ok;
    serialize_i64(_v: i64) -> Self::Ok;
    serialize_u8(_v: u8) -> Self::Ok;
    serialize_u16(_v: u16) -> Self::Ok;
    serialize_u32(_v: u32) -> Self::Ok;
    serialize_u64(_v: u64) -> Self::Ok;
    serialize_f32(_v: f32) -> Self::Ok;
    serialize_f64(_v: f64) -> Self::Ok;
    serialize_char(_v: char) -> Self::Ok;
    serialize_str(_v: &str) -> Self::Ok;
    serialize_bytes(_v: &[u8]) -> Self::Ok;
    serialize_none() -> Self::Ok;
    serialize_unit() -> Self::Ok;
    serialize_unit_struct(_name: &'static str) -> Self::Ok;
    serialize_seq(_len: Option<usize>) -> Self::SerializeSeq;
    serialize_tuple(_len: usize) -> Self::SerializeTuple;
    serialize_tuple_struct(_name: &'static str, _len: usize) -> Self::SerializeTupleStruct;
    serialize_tuple_variant(
      _name: &'static str, _index: u32, _variant: &'static str, _len: usize
    ) -> Self::SerializeTupleVariant;
    serialize_map(_len: Option<usize>) -> Self::SerializeMap;
    serialize_struct(_name: &'static str, _len: usize) -> Self::SerializeStruct;
    serialize_struct_variant(
      _name: &'static str, _index: u32, _variant: &'static str, _len: usize
    ) -> Self::SerializeStructVariant;
  }
  fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok, Self::Error> {
    unreachable!("SupportLang must serialize as a unit variant")
  }
  fn serialize_newtype_struct<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _value: &T,
  ) -> Result<Self::Ok, Self::Error> {
    unreachable!("SupportLang must serialize as a unit variant")
  }
  fn serialize_newtype_variant<T: ?Sized + Serialize>(
    self,
    _name: &'static str,
    _index: u32,
    _variant: &'static str,
    _value: &T,
  ) -> Result<Self::Ok, Self::Error> {
    unreachable!("SupportLang must serialize as a unit variant")
  }
}
