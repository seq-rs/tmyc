use std::borrow::Cow;
use std::cmp::Ordering;

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

use crate::patterns::{parse_yaml_float, parse_yaml_int};

/// A parsed YAML node, lifetime-bound to the original source string.
///
/// The lifetime parameter threads source borrows through the tree —
/// plain and unescaped scalars are `Cow::Borrowed` slices of the input;
/// only escapes, line folds, or owned construction force `Cow::Owned`.
///
/// `Value` is the intermediate representation between the parser and serde.
/// Users typically deal with [`from_str`](crate::from_str) /
/// [`to_string`](crate::to_string) and never see `Value` directly; reach
/// for it when you need to inspect or transform parsed YAML structurally
/// (e.g., normalising config data before deserializing into a struct).
///
/// # Variants
///
/// - [`Value::Null`] — explicit `null`/`~`/empty.
/// - [`Value::Bool`] — `true`/`false` per YAML 1.2 Core schema.
/// - [`Value::Int`] / [`Value::UInt`] — signed and unsigned integers.
///   `UInt` is preferred when the value fits; `Int` is used for negatives
///   or unsigned-overflow.
/// - [`Value::Float`] — including `.inf`/`-.inf`/`.nan`.
/// - [`Value::String`] — `Cow::Borrowed` zero-copy whenever possible.
/// - [`Value::Seq`] / [`Value::Map`] — block or flow containers.
/// - [`Value::Tagged`] — a value annotated with a non-standard tag, e.g.
///   `!myapp/Thing foo`. Standard tags (`!!str`, `!!int`, etc.) are
///   resolved at parse time and never appear as `Tagged`.
#[derive(Debug, Clone)]
pub enum Value<'a> {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(Cow<'a, str>),
    Seq(Vec<Value<'a>>),
    Map(Vec<(Value<'a>, Value<'a>)>),
    Tagged(Cow<'a, str>, Box<Value<'a>>),
}

/// Discriminant ordering for cross-variant comparisons.
/// Chosen so that "simpler" variants sort before more-complex ones.
fn discriminant_rank(v: &Value<'_>) -> u8 {
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

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value<'_> {}

impl Ord for Value<'_> {
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
            (Value::Tagged(ta, ia), Value::Tagged(tb, ib)) => {
                ta.cmp(tb).then_with(|| ia.cmp(ib))
            }
            _ => unreachable!("same rank implies same variant"),
        }
    }
}

impl PartialOrd for Value<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for Value<'_> {
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

/// Splice `<<: *alias` (or `<<: [*a, *b]`) entries into their containing map
///
/// Input:
/// ```yaml
/// service:
///   <<: *defaults
///   port: 443
/// ```
///
/// Output: `service`'s map has the keys from `*defaults` plus `port: 443`,
/// with the explicit `port` winning over any `port` in the defaults.
///
/// Semantics (YAML 1.1 merge key, used in 1.2 ecosystems):
/// - Explicit map keys win over merged-in keys.
/// - When `<<` value is a seq of maps, earlier entries win over later.
/// - `<<` with non-map / non-seq-of-map value is silently dropped.
/// - Recursion is bottom-up: child maps' merges resolve before the parent's.
pub(crate) fn resolve_merge_keys<'a>(v: Value<'a>) -> Value<'a> {
    match v {
        Value::Map(pairs) => {
            // Recurse first so any nested << gets expanded into the source
            // map before we splice it into this level.
            let recursed: Vec<(Value<'a>, Value<'a>)> = pairs
                .into_iter()
                .map(|(k, v)| (resolve_merge_keys(k), resolve_merge_keys(v)))
                .collect();
            merge_in_map(recursed)
        }
        Value::Seq(items) => Value::Seq(items.into_iter().map(resolve_merge_keys).collect()),
        Value::Tagged(tag, inner) => Value::Tagged(tag, Box::new(resolve_merge_keys(*inner))),
        other => other,
    }
}

fn merge_in_map<'a>(pairs: Vec<(Value<'a>, Value<'a>)>) -> Value<'a> {
    let mut result: Vec<(Value<'a>, Value<'a>)> = Vec::with_capacity(pairs.len());
    let mut sources: Vec<Vec<(Value<'a>, Value<'a>)>> = Vec::new();

    for (k, v) in pairs {
        let is_merge_key = matches!(&k, Value::String(s) if s == "<<");
        if is_merge_key {
            match v {
                Value::Map(inner) => sources.push(inner),
                Value::Seq(items) => {
                    for item in items {
                        if let Value::Map(inner) = item {
                            sources.push(inner);
                        }
                    }
                }
                _ => {} // non-map merge target: silently dropped per common practice
            }
        } else {
            result.push((k, v));
        }
    }

    for src in sources {
        for (k, v) in src {
            if !result.iter().any(|(rk, _)| rk == &k) {
                result.push((k, v));
            }
        }
    }
    Value::Map(result)
}

pub(crate) fn apply_tag<'a>(tag: Cow<'a, str>, inner: Value<'a>) -> Value<'a> {
    match tag.as_ref() {
        "!!str" => coerce_str(inner),
        "!!int" => coerce_int(inner),
        "!!float" => coerce_float(inner),
        "!!bool" => coerce_bool(inner),
        "!!null" => Value::Null,
        _ => Value::Tagged(tag, Box::new(inner)),
    }
}

fn coerce_str<'a>(v: Value<'a>) -> Value<'a> {
    use Value::*;
    match v {
        String(_) => v,
        Null => String(Cow::Borrowed("null")),
        Bool(true) => String(Cow::Borrowed("true")),
        Bool(false) => String(Cow::Borrowed("false")),
        Int(n) => String(Cow::Owned(n.to_string())),
        UInt(n) => String(Cow::Owned(n.to_string())),
        Float(n) => String(Cow::Owned(n.to_string())),
        other => other,
    }
}

fn coerce_int<'a>(v: Value<'a>) -> Value<'a> {
    use Value::*;
    match v {
        String(s) => parse_yaml_int(&s).unwrap_or(String(s)),
        Bool(true) => UInt(1),
        Bool(false) => UInt(0),
        v @ (Int(_) | UInt(_)) => v,
        Float(f) => Int(f as i64),
        other => other,
    }
}

fn coerce_float<'a>(v: Value<'a>) -> Value<'a> {
    use Value::*;
    match v {
        String(s) => parse_yaml_float(&s).map(Float).unwrap_or(String(s)),
        Bool(true) => Float(1.0),
        Bool(false) => Float(0.0),
        Int(i) => Float(i as f64),
        UInt(i) => Float(i as f64),
        other => other,
    }
}

fn coerce_bool<'a>(v: Value<'a>) -> Value<'a> {
    use Value::*;
    match v {
        String(s) if s.to_lowercase() == "true" => Bool(true),
        String(s) if s.to_lowercase() == "false" => Bool(false),
        UInt(0) | Int(0) => Bool(false),
        UInt(1) | Int(1) => Bool(true),
        Bool(_) => v,
        Float(0.0) => Bool(false),
        Float(1.0) => Bool(true),
        other => other,
    }
}

impl<'a> From<u8> for Value<'a> {
    fn from(value: u8) -> Self {
        Self::UInt(value as u64)
    }
}

impl<'a> From<u16> for Value<'a> {
    fn from(value: u16) -> Self {
        Self::UInt(value as u64)
    }
}

impl<'a> From<u32> for Value<'a> {
    fn from(value: u32) -> Self {
        Self::UInt(value as u64)
    }
}

impl<'a> From<u64> for Value<'a> {
    fn from(value: u64) -> Self {
        Self::UInt(value)
    }
}

impl<'a> From<u128> for Value<'a> {
    fn from(value: u128) -> Self {
        Self::UInt(value as u64)
    }
}

impl<'a> From<i8> for Value<'a> {
    fn from(value: i8) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<i16> for Value<'a> {
    fn from(value: i16) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<i32> for Value<'a> {
    fn from(value: i32) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<i64> for Value<'a> {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl<'a> From<i128> for Value<'a> {
    fn from(value: i128) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<f32> for Value<'a> {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl<'a> From<f64> for Value<'a> {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl<'a> From<bool> for Value<'a> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl<'a> From<&str> for Value<'a> {
    fn from(value: &str) -> Self {
        Self::String(Cow::Owned(value.to_string()))
    }
}

impl<'a> From<String> for Value<'a> {
    fn from(value: String) -> Self {
        Self::String(Cow::Owned(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // apply_tag dispatch

    #[test]
    fn apply_str_on_int() {
        let v = apply_tag(Cow::Borrowed("!!str"), Value::UInt(42));
        assert!(matches!(v, Value::String(s) if s == "42"));
    }
    #[test]
    fn apply_str_on_bool() {
        let v = apply_tag(Cow::Borrowed("!!str"), Value::Bool(true));
        assert!(matches!(v, Value::String(s) if s == "true"));
    }
    #[test]
    fn apply_str_on_null() {
        let v = apply_tag(Cow::Borrowed("!!str"), Value::Null);
        assert!(matches!(v, Value::String(s) if s == "null"));
    }
    #[test]
    fn apply_int_on_string_decimal() {
        let v = apply_tag(Cow::Borrowed("!!int"), Value::String(Cow::Borrowed("42")));
        assert!(matches!(v, Value::UInt(42)));
    }
    #[test]
    fn apply_int_on_string_hex() {
        let v = apply_tag(Cow::Borrowed("!!int"), Value::String(Cow::Borrowed("0xff")));
        assert!(matches!(v, Value::UInt(255)));
    }
    #[test]
    fn apply_int_on_string_garbage_passthrough() {
        let v = apply_tag(
            Cow::Borrowed("!!int"),
            Value::String(Cow::Borrowed("nonsense")),
        );
        assert!(matches!(v, Value::String(s) if s == "nonsense"));
    }
    #[test]
    fn apply_float_on_string() {
        let v = apply_tag(
            Cow::Borrowed("!!float"),
            Value::String(Cow::Borrowed("3.1")),
        );
        assert!(matches!(v, Value::Float(f) if (f - 3.1).abs() < 1e-9));
    }
    #[test]
    fn apply_float_on_string_inf() {
        let v = apply_tag(
            Cow::Borrowed("!!float"),
            Value::String(Cow::Borrowed(".inf")),
        );
        assert!(matches!(v, Value::Float(f) if f == f64::INFINITY));
    }
    #[test]
    fn apply_float_on_int_promotes() {
        let v = apply_tag(Cow::Borrowed("!!float"), Value::UInt(7));
        assert!(matches!(v, Value::Float(f) if f == 7.0));
    }
    #[test]
    fn apply_bool_on_string_case_insensitive() {
        let v = apply_tag(
            Cow::Borrowed("!!bool"),
            Value::String(Cow::Borrowed("TRUE")),
        );
        assert!(matches!(v, Value::Bool(true)));
        let v = apply_tag(
            Cow::Borrowed("!!bool"),
            Value::String(Cow::Borrowed("False")),
        );
        assert!(matches!(v, Value::Bool(false)));
    }
    #[test]
    fn apply_null_drops_inner() {
        let v = apply_tag(
            Cow::Borrowed("!!null"),
            Value::String(Cow::Borrowed("ignored")),
        );
        assert!(matches!(v, Value::Null));
    }
    #[test]
    fn apply_custom_tag_wraps() {
        let v = apply_tag(
            Cow::Borrowed("!myapp/Thing"),
            Value::String(Cow::Borrowed("x")),
        );
        match v {
            Value::Tagged(tag, inner) => {
                assert_eq!(tag, "!myapp/Thing");
                assert!(matches!(*inner, Value::String(s) if s == "x"));
            }
            other => panic!("expected Tagged, got {other:?}"),
        }
    }
    #[test]
    fn apply_verbatim_tag_wraps() {
        let v = apply_tag(Cow::Borrowed("!<tag:example.com,2026:foo>"), Value::UInt(5));
        match v {
            Value::Tagged(tag, inner) => {
                assert_eq!(tag, "!<tag:example.com,2026:foo>");
                assert!(matches!(*inner, Value::UInt(5)));
            }
            other => panic!("expected Tagged, got {other:?}"),
        }
    }

    // coerce_int edge cases not covered through apply_tag above

    #[test]
    fn coerce_int_from_bool() {
        assert!(matches!(coerce_int(Value::Bool(true)), Value::UInt(1)));
        assert!(matches!(coerce_int(Value::Bool(false)), Value::UInt(0)));
    }
    #[test]
    fn coerce_int_from_float_truncates() {
        assert!(matches!(coerce_int(Value::Float(3.7)), Value::Int(3)));
        assert!(matches!(coerce_int(Value::Float(-2.9)), Value::Int(-2)));
    }
    #[test]
    fn coerce_int_passthrough_on_uint() {
        assert!(matches!(coerce_int(Value::UInt(42)), Value::UInt(42)));
    }

    // coerce_float edge cases

    #[test]
    fn coerce_float_from_int() {
        assert!(matches!(coerce_float(Value::Int(-7)), Value::Float(f) if f == -7.0));
        assert!(matches!(coerce_float(Value::UInt(7)), Value::Float(f) if f == 7.0));
    }
    #[test]
    fn coerce_float_from_bool() {
        assert!(matches!(coerce_float(Value::Bool(true)), Value::Float(f) if f == 1.0));
        assert!(matches!(coerce_float(Value::Bool(false)), Value::Float(f) if f == 0.0));
    }
    #[test]
    fn coerce_float_identity() {
        assert!(matches!(coerce_float(Value::Float(2.5)), Value::Float(f) if f == 2.5));
    }

    // coerce_bool edge cases (Norway-problem stance)

    #[test]
    fn coerce_bool_from_zero_one() {
        assert!(matches!(coerce_bool(Value::UInt(0)), Value::Bool(false)));
        assert!(matches!(coerce_bool(Value::UInt(1)), Value::Bool(true)));
        assert!(matches!(coerce_bool(Value::Int(0)), Value::Bool(false)));
        assert!(matches!(coerce_bool(Value::Int(1)), Value::Bool(true)));
    }
    #[test]
    fn coerce_bool_non_binary_int_passthrough() {
        assert!(matches!(coerce_bool(Value::UInt(2)), Value::UInt(2)));
        assert!(matches!(coerce_bool(Value::Int(-1)), Value::Int(-1)));
    }
    #[test]
    fn coerce_bool_yes_no_not_coerced() {
        // YAML 1.2 stance: only true/false (case variants) are bools
        let v = coerce_bool(Value::String(Cow::Borrowed("yes")));
        assert!(matches!(v, Value::String(s) if s == "yes"));
        let v = coerce_bool(Value::String(Cow::Borrowed("NO")));
        assert!(matches!(v, Value::String(s) if s == "NO"));
    }

    // coerce_str edge cases

    #[test]
    fn coerce_str_from_float() {
        let v = coerce_str(Value::Float(3.5));
        assert!(matches!(v, Value::String(s) if s == "3.5"));
    }
    #[test]
    fn coerce_str_identity_on_string() {
        let v = coerce_str(Value::String(Cow::Borrowed("hi")));
        assert!(matches!(v, Value::String(s) if s == "hi"));
    }
}
