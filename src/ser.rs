use std::borrow::Cow;

use crate::{Error, Value};

pub struct ValueSerializer;
use Value::*;
use serde::{Serialize, ser::Error as _};

impl serde::Serializer for ValueSerializer {
    type Ok = Value<'static>;
    type SerializeSeq = SerializeSeqImpl;
    type SerializeMap = SerializeMapImpl;
    type SerializeStruct = SerializeMapImpl;
    type SerializeStructVariant = SerializeStructVariantImpl;
    type SerializeTuple = SerializeSeqImpl;
    type SerializeTupleStruct = SerializeSeqImpl;
    type SerializeTupleVariant = SerializeTupleVariantImpl;
    type Error = Error;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeSeqImpl(Vec::with_capacity(len.unwrap_or(0))))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Ok(String(Cow::Owned(v.to_string())))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Seq(v.iter().map(|b| UInt(*b as u64)).collect()))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(SerializeSeqImpl(Vec::with_capacity(len)))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(SerializeMapImpl {
            pairs: Vec::with_capacity(len),
            pending_key: None,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMapImpl {
            pairs: Vec::with_capacity(len.unwrap_or(0)),
            pending_key: None,
        })
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(String(Cow::Owned(variant.to_string())))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(ValueSerializer)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let inner = value.serialize(ValueSerializer)?;
        Ok(Map(vec![(
            String(Cow::Owned(variant.to_string())),
            inner,
        )]))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(SerializeSeqImpl(Vec::with_capacity(len)))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(SerializeTupleVariantImpl {
            variant,
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeStructVariantImpl {
            variant,
            pairs: Vec::with_capacity(len),
        })
    }
}

pub struct SerializeSeqImpl(Vec<Value<'static>>);

impl serde::ser::SerializeSeq for SerializeSeqImpl {
    type Ok = Value<'static>;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.0.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Seq(self.0))
    }
}

impl serde::ser::SerializeTuple for SerializeSeqImpl {
    type Ok = Value<'static>;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.0.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Seq(self.0))
    }
}

impl serde::ser::SerializeTupleStruct for SerializeSeqImpl {
    type Ok = Value<'static>;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.0.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Seq(self.0))
    }
}

pub struct SerializeTupleVariantImpl {
    variant: &'static str,
    items: Vec<Value<'static>>,
}

impl serde::ser::SerializeTupleVariant for SerializeTupleVariantImpl {
    type Ok = Value<'static>;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // Wrap as single-key map so deserializer can recover the variant name
        Ok(Map(vec![(
            String(Cow::Owned(self.variant.to_string())),
            Seq(self.items),
        )]))
    }
}

pub struct SerializeMapImpl {
    pairs: Vec<(Value<'static>, Value<'static>)>,
    pending_key: Option<Value<'static>>,
}

impl serde::ser::SerializeMap for SerializeMapImpl {
    type Ok = Value<'static>;
    type Error = Error;
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.pending_key = Some(key.serialize(ValueSerializer)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let k = self
            .pending_key
            .take()
            .ok_or_else(|| Error::custom("value without key"))?;
        self.pairs.push((k, value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Map(self.pairs))
    }
}

impl serde::ser::SerializeStruct for SerializeMapImpl {
    type Ok = Value<'static>;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.pairs
            .push((key.into(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Map(self.pairs))
    }
}

pub struct SerializeStructVariantImpl {
    variant: &'static str,
    pairs: Vec<(Value<'static>, Value<'static>)>,
}

impl serde::ser::SerializeStructVariant for SerializeStructVariantImpl {
    type Ok = Value<'static>;
    type Error = Error;

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.pairs.push((
            String(Cow::Owned(key.to_string())),
            value.serialize(ValueSerializer)?,
        ));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Map(vec![(
            String(Cow::Owned(self.variant.to_string())),
            Map(self.pairs),
        )]))
    }
}

/// Serialize a value to a [`Value`].
///
/// This is the low-level entry point — use [`to_string`](crate::to_string)
/// for a complete YAML string. Returning a `Value` first is useful for
/// inspection or transformation before emission.
///
/// # Example
///
/// ```
/// use serde::Serialize;
/// use tmyc::{to_value, Value};
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let v = to_value(&Point { x: 1, y: 2 }).unwrap();
/// assert!(matches!(v, Value::Map(_)));
/// ```
pub fn to_value<T: ?Sized + Serialize>(v: &T) -> crate::Result<Value<'static>> {
    v.serialize(ValueSerializer)
}
