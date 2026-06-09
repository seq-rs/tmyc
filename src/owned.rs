use std::{borrow::Cow, cmp::Ordering};

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};

use crate::BorrowedValue;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Seq(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Tagged(String, Box<Value>),
}

impl From<BorrowedValue<'_>> for Value {
    fn from(v: BorrowedValue<'_>) -> Self {
        match v {
            BorrowedValue::String(c) => Value::String(c.into_owned()),
            BorrowedValue::Null => Value::Null,
            BorrowedValue::Bool(b) => Value::Bool(b),
            BorrowedValue::Int(n) => Value::Int(n),
            BorrowedValue::UInt(n) => Value::UInt(n),
            BorrowedValue::Float(f) => Value::Float(f),
            BorrowedValue::Seq(seq) => Value::Seq(seq.into_iter().map(Value::from).collect()),
            BorrowedValue::Map(kv) => {
                Value::Map(kv.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
            }
            BorrowedValue::Tagged(tag, value) => {
                Value::Tagged(tag.into_owned(), Box::new((*value).into()))
            }
        }
    }
}

impl<'a> From<&'a Value> for BorrowedValue<'a> {
    fn from(value: &'a Value) -> Self {
        match value {
            Value::Null => BorrowedValue::Null,
            Value::Bool(b) => BorrowedValue::Bool(*b),
            Value::Int(n) => BorrowedValue::Int(*n),
            Value::UInt(n) => BorrowedValue::UInt(*n),
            Value::Float(f) => BorrowedValue::Float(*f),
            Value::String(s) => BorrowedValue::String(Cow::Borrowed(s)),
            Value::Seq(vals) => BorrowedValue::Seq(vals.iter().map(Into::into).collect()),
            Value::Map(kv) => {
                BorrowedValue::Map(kv.iter().map(|(k, v)| (k.into(), v.into())).collect())
            }
            Value::Tagged(tag, val) => {
                BorrowedValue::Tagged(Cow::Borrowed(tag), Box::new((&**val).into()))
            }
        }
    }
}

/// Discriminant ordering for cross-variant comparisons.
/// Chosen so that "simpler" variants sort before more-complex ones.
fn discriminant_rank(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Int(_) => 2,
        Value::UInt(_) => 3,
        Value::Float(_) => 4,
        Value::String(_) => 5,
        Value::Seq(_) => 6,
        Value::Map(_) => 7,
        Value::Tagged(_, _) => 8,
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value {}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        let ra = discriminant_rank(self);
        let rb = discriminant_rank(other);
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (self, other) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::UInt(a), Value::UInt(b)) => a.cmp(b),
            // total_cmp gives a total order including NaN (NaN compares Equal to NaN)
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Seq(a), Value::Seq(b)) => a.cmp(b),
            (Value::Map(a), Value::Map(b)) => a.cmp(b),
            (Value::Tagged(ta, ia), Value::Tagged(tb, ib)) => ta.cmp(tb).then_with(|| ia.cmp(ib)),
            _ => unreachable!("same rank implies same variant"),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Self::UInt(value as u64)
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Self::UInt(value as u64)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::UInt(value as u64)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self::UInt(value)
    }
}

impl From<u128> for Value {
    fn from(value: u128) -> Self {
        Self::UInt(value as u64)
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Self::Int(value as i64)
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Self::Int(value as i64)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Int(value as i64)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i128> for Value {
    fn from(value: i128) -> Self {
        Self::Int(value as i64)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(n) => serializer.serialize_i64(*n),
            Value::UInt(n) => serializer.serialize_u64(*n),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::String(s) => serializer.serialize_str(s),
            Value::Seq(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::Map(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            // Custom tags are transparent on serialize — emit the inner value.
            // Round-tripping the tag itself is a non-goal of the pragmatic emitter.
            Value::Tagged(_, inner) => inner.serialize(serializer),
        }
    }
}
