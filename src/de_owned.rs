//! `Deserialize` for the owned [`Value`].
//!
//! This is the mirror of `serde_json::Value`'s own `Deserialize`: it lets the
//! owned [`Value`] be a deserialization *target*, so a struct field can be
//! typed `yaml0::Value` and `let v: Value = yaml0::from_str(s)?` works.
//!
//! Note the direction. `de.rs` implements a [`Deserializer`] *for*
//! `&BorrowedValue` — it drives some `T` from an existing value. This file
//! implements the opposite trait, [`Deserialize`], via a [`Visitor`] that
//! *builds* an owned tree from whatever deserializer is feeding it. The two
//! compose: parse → `BorrowedValue` → this visitor → owned `Value`.
//!
//! `Value::Tagged` is never produced here: serde has no concept of a YAML tag,
//! so the visitor is never told about one. Tags only arise from parsing into
//! `BorrowedValue`, and the `&BorrowedValue` deserializer renders them
//! transparently (the inner value is visited, the tag dropped).
//!
//! ```
//! let v: yaml0::Value = yaml0::from_str("a: 1\nb: [x, y]\n").unwrap();
//! assert!(matches!(v, yaml0::Value::Map(_)));
//! ```

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::Value;

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any YAML value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    // Signed and unsigned stay distinct, matching the parser's
    // resolve preference (unsigned when the value is non-negative).
    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Int(v))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(i64::try_from(v)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::String(v.to_string())))
    }

    // Other deserializers (e.g. serde_json) may hand back 128-bit ints.
    // Narrow to the supported width or fail loudly rather than truncate.
    fn visit_i128<E: de::Error>(self, v: i128) -> Result<Value, E> {
        Ok(i64::try_from(v)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::String(v.to_string())))
    }
    fn visit_u128<E: de::Error>(self, v: u128) -> Result<Value, E> {
        Ok(i64::try_from(v)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::String(v.to_string())))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Value, E> {
        Ok(Value::Float(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }
    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(v) = seq.next_element()? {
            items.push(v);
        }
        Ok(Value::Seq(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut pairs = Vec::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((k, v)) = map.next_entry()? {
            pairs.push((k, v));
        }
        Ok(Value::Map(pairs))
    }
}
