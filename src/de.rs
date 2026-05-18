use std::borrow::Cow;

use serde::{Deserialize, de::{DeserializeOwned, Error as _, IntoDeserializer, VariantAccess}};

use crate::{Error, Parser, Value};

impl<'de> serde::de::Deserializer<'de> for &'de Value<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(*b),
            Value::Int(i) => visitor.visit_i64(*i),
            Value::UInt(i) => visitor.visit_u64(*i),
            Value::Float(f) => visitor.visit_f64(*f),
            // zero-copy
            Value::String(Cow::Borrowed(s)) => visitor.visit_borrowed_str(s),
            Value::String(Cow::Owned(s)) => visitor.visit_str(s),
            Value::Seq(items) => visitor.visit_seq(SeqAccessImpl::new(items)),
            Value::Map(pairs) => visitor.visit_map(MapAccessImpl::new(pairs)),
            Value::Tagged(_, value) => value.as_ref().deserialize_any(visitor),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        str string bytes byte_buf seq map struct tuple tuple_struct
        identifier ignored_any
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::Null => visitor.visit_unit(),
            _ => Err(Error::custom("expected unit")),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {

            Value::String(s) => visitor.visit_enum(s.as_ref().into_deserializer()),
            Value::Map(pairs) if pairs.len() == 1 => {
                visitor.visit_enum(EnumAccessImpl { key: &pairs[0].0, value: &pairs[0].1 })
            }
            _ => Err(Error::custom("expected enum (string or single-key map)")),
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
}

pub struct SeqAccessImpl<'de> {
    iter: std::slice::Iter<'de, Value<'de>>,
}

impl<'de> SeqAccessImpl<'de> {
    fn new(items: &'de Vec<Value<'de>>) -> Self {
        Self { iter: items.iter() }
    }
}

impl<'de> serde::de::SeqAccess<'de> for SeqAccessImpl<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(v) => seed.deserialize(v).map(Some),
            None => Ok(None),
        }
    }
}

pub struct MapAccessImpl<'de> {
    iter: std::slice::Iter<'de, (Value<'de>, Value<'de>)>,
    pending_value: Option<&'de Value<'de>>,
}

impl<'de> MapAccessImpl<'de> {
    pub fn new(pairs: &'de Vec<(Value<'de>, Value<'de>)>) -> Self {
        Self {
            iter: pairs.iter(),
            pending_value: None,
        }
    }
}

impl<'de> serde::de::MapAccess<'de> for MapAccessImpl<'de> {
    type Error = Error;
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((k, v)) => {
                self.pending_value = Some(v);
                seed.deserialize(k).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let v = self
            .pending_value
            .take()
            .expect("next_value_seed without prior key");
        seed.deserialize(v)
    }
}

pub struct EnumAccessImpl<'de> {
    key: &'de Value<'de>,
    value: &'de Value<'de>,
}

impl<'de> serde::de::EnumAccess<'de> for EnumAccessImpl<'de> {
    type Error = Error;
    type Variant = VariantAccessImpl<'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
        where
            V: serde::de::DeserializeSeed<'de> {
        let variant = seed.deserialize(self.key)?;
        Ok((variant, VariantAccessImpl { value: self.value }))
    }
}


pub struct VariantAccessImpl<'de> {
    value: &'de Value<'de>,
}

impl<'de> VariantAccess<'de> for VariantAccessImpl<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            Value::Null => Ok(()),
                _ => Err(Error::custom("expected unit variant payload to be null"))
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
        where
            T: serde::de::DeserializeSeed<'de> {
        seed.deserialize(self.value)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de> {
        match self.value {
            Value::Seq(items) => visitor.visit_seq(SeqAccessImpl::new(items)),
            _ => Err(Error::custom("expected tuple variant (seq)"))
        }
    }

    fn struct_variant<V>(
            self,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de> {
        match self.value {
            Value::Map(pairs) => visitor.visit_map(MapAccessImpl::new(pairs)),
            _ => Err(Error::custom("expected struct variant (map)")),
        }
    }
}

/// Deserialize from a pre-parsed [`Value`].
///
/// Use this when you want zero-copy borrows into the original input
/// (struct fields typed as `&str` or `Cow<'de, str>`). The caller owns
/// the `Value`, so its lifetime is what `T`'s borrowed fields bind to.
///
/// # Example
///
/// ```
/// use serde::Deserialize;
/// use tmyc::{from_value, Parser};
///
/// #[derive(Deserialize)]
/// struct Cfg<'a> { name: &'a str }
///
/// let src = "name: foo\n";
/// let value = Parser::new(src).parse().unwrap();
/// let cfg: Cfg<'_> = from_value(&value).unwrap();
/// assert_eq!(cfg.name, "foo");
/// ```
pub fn from_value<'de, T: Deserialize<'de>>(value: &'de Value<'de>) -> crate::Result<T> {
    T::deserialize(value)
}

/// Parse a YAML string and deserialize the single document into `T`.
///
/// `T` must be [`DeserializeOwned`] — it cannot hold borrowed fields,
/// because the intermediate [`Value`] is local to this function. For
/// borrowed deserialization use [`from_value`] after parsing manually.
///
/// Errors if the input contains more than one document. Use
/// [`Parser::parse_all`] for multi-document streams.
///
/// # Example
///
/// ```
/// use serde::Deserialize;
///
/// #[derive(Deserialize, PartialEq, Debug)]
/// struct Cfg { name: String, port: u16 }
///
/// let cfg: Cfg = tmyc::from_str("name: web\nport: 8080\n").unwrap();
/// assert_eq!(cfg, Cfg { name: "web".into(), port: 8080 });
/// ```
pub fn from_str<T: DeserializeOwned>(s: &str) -> crate::Result<T> {
    let value = Parser::new(s).parse()?;
    T::deserialize(&value)
}
