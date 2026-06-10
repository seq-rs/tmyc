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
/// `BorrowedValue` is the borrowed, zero-copy form of the data model — the
/// parser's native output and the emitter's input. Most code goes through
/// [`from_str`](crate::from_str) / [`to_string`](crate::to_string) and never
/// touches it; reach for it when you want to inspect or transform parsed YAML
/// structurally, or to deserialize into `&str` fields that slice the input.
/// For a lifetime-free value you can store in a struct, use the owned
/// [`Value`](crate::Value) instead; convert with `From` in either direction.
///
/// # Variants
///
/// - [`BorrowedValue::Null`] — explicit `null`/`~`/empty.
/// - [`BorrowedValue::Bool`] — `true`/`false` per YAML 1.2 Core schema.
/// - [`BorrowedValue::Int`] — signed and unsigned integers.
/// - [`BorrowedValue::Float`] — including `.inf`/`-.inf`/`.nan`.
/// - [`BorrowedValue::String`] — `Cow::Borrowed` zero-copy whenever possible.
/// - [`BorrowedValue::Seq`] / [`BorrowedValue::Map`] — block or flow containers.
/// - [`BorrowedValue::Tagged`] — a value annotated with a non-standard tag, e.g.
///   `!myapp/Thing foo`. Standard tags (`!!str`, `!!int`, etc.) are
///   resolved at parse time and never appear as `Tagged`.
#[derive(Debug, Clone)]
pub enum BorrowedValue<'a> {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Cow<'a, str>),
    Seq(Vec<BorrowedValue<'a>>),
    Map(Vec<(BorrowedValue<'a>, BorrowedValue<'a>)>),
    Tagged(Cow<'a, str>, Box<BorrowedValue<'a>>),
}

/// Discriminant ordering for cross-variant comparisons.
/// Chosen so that "simpler" variants sort before more-complex ones.
fn discriminant_rank(v: &BorrowedValue<'_>) -> u8 {
    match v {
        BorrowedValue::Null => 0,
        BorrowedValue::Bool(_) => 1,
        BorrowedValue::Int(_) => 2,
        BorrowedValue::Float(_) => 4,
        BorrowedValue::String(_) => 5,
        BorrowedValue::Seq(_) => 6,
        BorrowedValue::Map(_) => 7,
        BorrowedValue::Tagged(_, _) => 8,
    }
}

impl PartialEq for BorrowedValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for BorrowedValue<'_> {}

impl Ord for BorrowedValue<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let ra = discriminant_rank(self);
        let rb = discriminant_rank(other);
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (self, other) {
            (BorrowedValue::Null, BorrowedValue::Null) => Ordering::Equal,
            (BorrowedValue::Bool(a), BorrowedValue::Bool(b)) => a.cmp(b),
            (BorrowedValue::Int(a), BorrowedValue::Int(b)) => a.cmp(b),
            // total_cmp gives a total order including NaN (NaN compares Equal to NaN)
            (BorrowedValue::Float(a), BorrowedValue::Float(b)) => a.total_cmp(b),
            (BorrowedValue::String(a), BorrowedValue::String(b)) => a.cmp(b),
            (BorrowedValue::Seq(a), BorrowedValue::Seq(b)) => a.cmp(b),
            (BorrowedValue::Map(a), BorrowedValue::Map(b)) => a.cmp(b),
            (BorrowedValue::Tagged(ta, ia), BorrowedValue::Tagged(tb, ib)) => {
                ta.cmp(tb).then_with(|| ia.cmp(ib))
            }
            _ => unreachable!("same rank implies same variant"),
        }
    }
}

impl PartialOrd for BorrowedValue<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for BorrowedValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            BorrowedValue::Null => serializer.serialize_unit(),
            BorrowedValue::Bool(b) => serializer.serialize_bool(*b),
            BorrowedValue::Int(n) => serializer.serialize_i64(*n),
            BorrowedValue::Float(f) => serializer.serialize_f64(*f),
            BorrowedValue::String(s) => serializer.serialize_str(s),
            BorrowedValue::Seq(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            BorrowedValue::Map(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            // Custom tags are transparent on serialize — emit the inner value.
            // Round-tripping the tag itself is a non-goal of the pragmatic emitter.
            BorrowedValue::Tagged(_, inner) => inner.serialize(serializer),
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
pub(crate) fn resolve_merge_keys<'a>(v: BorrowedValue<'a>) -> BorrowedValue<'a> {
    match v {
        BorrowedValue::Map(pairs) => {
            // Recurse first so any nested << gets expanded into the source
            // map before we splice it into this level.
            let recursed: Vec<(BorrowedValue<'a>, BorrowedValue<'a>)> = pairs
                .into_iter()
                .map(|(k, v)| (resolve_merge_keys(k), resolve_merge_keys(v)))
                .collect();
            merge_in_map(recursed)
        }
        BorrowedValue::Seq(items) => BorrowedValue::Seq(items.into_iter().map(resolve_merge_keys).collect()),
        BorrowedValue::Tagged(tag, inner) => BorrowedValue::Tagged(tag, Box::new(resolve_merge_keys(*inner))),
        other => other,
    }
}

fn merge_in_map<'a>(pairs: Vec<(BorrowedValue<'a>, BorrowedValue<'a>)>) -> BorrowedValue<'a> {
    let mut result: Vec<(BorrowedValue<'a>, BorrowedValue<'a>)> = Vec::with_capacity(pairs.len());
    let mut sources: Vec<Vec<(BorrowedValue<'a>, BorrowedValue<'a>)>> = Vec::new();

    for (k, v) in pairs {
        let is_merge_key = matches!(&k, BorrowedValue::String(s) if s == "<<");
        if is_merge_key {
            match v {
                BorrowedValue::Map(inner) => sources.push(inner),
                BorrowedValue::Seq(items) => {
                    for item in items {
                        if let BorrowedValue::Map(inner) = item {
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
    BorrowedValue::Map(result)
}

pub(crate) fn apply_tag<'a>(tag: Cow<'a, str>, inner: BorrowedValue<'a>) -> BorrowedValue<'a> {
    match tag.as_ref() {
        "!!str" => coerce_str(inner),
        "!!int" => coerce_int(inner),
        "!!float" => coerce_float(inner),
        "!!bool" => coerce_bool(inner),
        "!!null" => BorrowedValue::Null,
        _ => BorrowedValue::Tagged(tag, Box::new(inner)),
    }
}

fn coerce_str<'a>(v: BorrowedValue<'a>) -> BorrowedValue<'a> {
    use BorrowedValue::*;
    match v {
        String(_) => v,
        Null => String(Cow::Borrowed("null")),
        Bool(true) => String(Cow::Borrowed("true")),
        Bool(false) => String(Cow::Borrowed("false")),
        Int(n) => String(Cow::Owned(n.to_string())),
        Float(n) => String(Cow::Owned(n.to_string())),
        other => other,
    }
}

fn coerce_int<'a>(v: BorrowedValue<'a>) -> BorrowedValue<'a> {
    use BorrowedValue::*;
    match v {
        String(s) => parse_yaml_int(&s).unwrap_or(String(s)),
        Bool(true) => Int(1),
        Bool(false) => Int(0),
        v @ Int(_) => v,
        Float(f) => Int(f as i64),
        other => other,
    }
}

fn coerce_float<'a>(v: BorrowedValue<'a>) -> BorrowedValue<'a> {
    use BorrowedValue::*;
    match v {
        String(s) => parse_yaml_float(&s).map(Float).unwrap_or(String(s)),
        Bool(true) => Float(1.0),
        Bool(false) => Float(0.0),
        Int(i) => Float(i as f64),
        other => other,
    }
}

fn coerce_bool<'a>(v: BorrowedValue<'a>) -> BorrowedValue<'a> {
    use BorrowedValue::*;
    match v {
        String(s) if s.to_lowercase() == "true" => Bool(true),
        String(s) if s.to_lowercase() == "false" => Bool(false),
        Int(0) => Bool(false),
        Int(1) => Bool(true),
        Bool(_) => v,
        Float(0.0) => Bool(false),
        Float(1.0) => Bool(true),
        other => other,
    }
}

impl<'a> From<u8> for BorrowedValue<'a> {
    fn from(value: u8) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<u16> for BorrowedValue<'a> {
    fn from(value: u16) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<u32> for BorrowedValue<'a> {
    fn from(value: u32) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<u64> for BorrowedValue<'a> {
    fn from(value: u64) -> Self {
        i64::try_from(value).map(Self::Int)
            .unwrap_or_else(|_| Self::String(Cow::Owned(value.to_string())))
    }
}

impl<'a> From<u128> for BorrowedValue<'a> {
    fn from(value: u128) -> Self {
        i64::try_from(value).map(Self::Int)
            .unwrap_or_else(|_| Self::String(Cow::Owned(value.to_string())))
    }
}

impl<'a> From<i8> for BorrowedValue<'a> {
    fn from(value: i8) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<i16> for BorrowedValue<'a> {
    fn from(value: i16) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<i32> for BorrowedValue<'a> {
    fn from(value: i32) -> Self {
        Self::Int(value as i64)
    }
}

impl<'a> From<i64> for BorrowedValue<'a> {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl<'a> From<i128> for BorrowedValue<'a> {
    fn from(value: i128) -> Self {
        i64::try_from(value).map(Self::Int)
            .unwrap_or_else(|_| Self::String(Cow::Owned(value.to_string())))
    }
}

impl<'a> From<f32> for BorrowedValue<'a> {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl<'a> From<f64> for BorrowedValue<'a> {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl<'a> From<bool> for BorrowedValue<'a> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl<'a> From<&str> for BorrowedValue<'a> {
    fn from(value: &str) -> Self {
        Self::String(Cow::Owned(value.to_string()))
    }
}

impl<'a> From<String> for BorrowedValue<'a> {
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
        let v = apply_tag(Cow::Borrowed("!!str"), BorrowedValue::Int(42));
        assert!(matches!(v, BorrowedValue::String(s) if s == "42"));
    }
    #[test]
    fn apply_str_on_bool() {
        let v = apply_tag(Cow::Borrowed("!!str"), BorrowedValue::Bool(true));
        assert!(matches!(v, BorrowedValue::String(s) if s == "true"));
    }
    #[test]
    fn apply_str_on_null() {
        let v = apply_tag(Cow::Borrowed("!!str"), BorrowedValue::Null);
        assert!(matches!(v, BorrowedValue::String(s) if s == "null"));
    }
    #[test]
    fn apply_int_on_string_decimal() {
        let v = apply_tag(Cow::Borrowed("!!int"), BorrowedValue::String(Cow::Borrowed("42")));
        assert!(matches!(v, BorrowedValue::Int(42)));
    }
    #[test]
    fn apply_int_on_string_hex() {
        let v = apply_tag(Cow::Borrowed("!!int"), BorrowedValue::String(Cow::Borrowed("0xff")));
        assert!(matches!(v, BorrowedValue::Int(255)));
    }
    #[test]
    fn apply_int_on_string_garbage_passthrough() {
        let v = apply_tag(
            Cow::Borrowed("!!int"),
            BorrowedValue::String(Cow::Borrowed("nonsense")),
        );
        assert!(matches!(v, BorrowedValue::String(s) if s == "nonsense"));
    }
    #[test]
    fn apply_float_on_string() {
        let v = apply_tag(
            Cow::Borrowed("!!float"),
            BorrowedValue::String(Cow::Borrowed("3.1")),
        );
        assert!(matches!(v, BorrowedValue::Float(f) if (f - 3.1).abs() < 1e-9));
    }
    #[test]
    fn apply_float_on_string_inf() {
        let v = apply_tag(
            Cow::Borrowed("!!float"),
            BorrowedValue::String(Cow::Borrowed(".inf")),
        );
        assert!(matches!(v, BorrowedValue::Float(f) if f == f64::INFINITY));
    }
    #[test]
    fn apply_float_on_int_promotes() {
        let v = apply_tag(Cow::Borrowed("!!float"), BorrowedValue::Int(7));
        assert!(matches!(v, BorrowedValue::Float(f) if f == 7.0));
    }
    #[test]
    fn apply_bool_on_string_case_insensitive() {
        let v = apply_tag(
            Cow::Borrowed("!!bool"),
            BorrowedValue::String(Cow::Borrowed("TRUE")),
        );
        assert!(matches!(v, BorrowedValue::Bool(true)));
        let v = apply_tag(
            Cow::Borrowed("!!bool"),
            BorrowedValue::String(Cow::Borrowed("False")),
        );
        assert!(matches!(v, BorrowedValue::Bool(false)));
    }
    #[test]
    fn apply_null_drops_inner() {
        let v = apply_tag(
            Cow::Borrowed("!!null"),
            BorrowedValue::String(Cow::Borrowed("ignored")),
        );
        assert!(matches!(v, BorrowedValue::Null));
    }
    #[test]
    fn apply_custom_tag_wraps() {
        let v = apply_tag(
            Cow::Borrowed("!myapp/Thing"),
            BorrowedValue::String(Cow::Borrowed("x")),
        );
        match v {
            BorrowedValue::Tagged(tag, inner) => {
                assert_eq!(tag, "!myapp/Thing");
                assert!(matches!(*inner, BorrowedValue::String(s) if s == "x"));
            }
            other => panic!("expected Tagged, got {other:?}"),
        }
    }
    #[test]
    fn apply_verbatim_tag_wraps() {
        let v = apply_tag(Cow::Borrowed("!<tag:example.com,2026:foo>"), BorrowedValue::Int(5));
        match v {
            BorrowedValue::Tagged(tag, inner) => {
                assert_eq!(tag, "!<tag:example.com,2026:foo>");
                assert!(matches!(*inner, BorrowedValue::Int(5)));
            }
            other => panic!("expected Tagged, got {other:?}"),
        }
    }

    // coerce_int edge cases not covered through apply_tag above

    #[test]
    fn coerce_int_from_bool() {
        assert!(matches!(coerce_int(BorrowedValue::Bool(true)), BorrowedValue::Int(1)));
        assert!(matches!(coerce_int(BorrowedValue::Bool(false)), BorrowedValue::Int(0)));
    }
    #[test]
    fn coerce_int_from_float_truncates() {
        assert!(matches!(coerce_int(BorrowedValue::Float(3.7)), BorrowedValue::Int(3)));
        assert!(matches!(coerce_int(BorrowedValue::Float(-2.9)), BorrowedValue::Int(-2)));
    }
    // coerce_float edge cases

    #[test]
    fn coerce_float_from_int() {
        assert!(matches!(coerce_float(BorrowedValue::Int(-7)), BorrowedValue::Float(f) if f == -7.0));
        assert!(matches!(coerce_float(BorrowedValue::Int(7)), BorrowedValue::Float(f) if f == 7.0));
    }
    #[test]
    fn coerce_float_from_bool() {
        assert!(matches!(coerce_float(BorrowedValue::Bool(true)), BorrowedValue::Float(f) if f == 1.0));
        assert!(matches!(coerce_float(BorrowedValue::Bool(false)), BorrowedValue::Float(f) if f == 0.0));
    }
    #[test]
    fn coerce_float_identity() {
        assert!(matches!(coerce_float(BorrowedValue::Float(2.5)), BorrowedValue::Float(f) if f == 2.5));
    }

    // coerce_bool edge cases (Norway-problem stance)

    #[test]
    fn coerce_bool_from_zero_one() {
        assert!(matches!(coerce_bool(BorrowedValue::Int(0)), BorrowedValue::Bool(false)));
        assert!(matches!(coerce_bool(BorrowedValue::Int(1)), BorrowedValue::Bool(true)));
    }
    #[test]
    fn coerce_bool_non_binary_int_passthrough() {
        assert!(matches!(coerce_bool(BorrowedValue::Int(2)), BorrowedValue::Int(2)));
        assert!(matches!(coerce_bool(BorrowedValue::Int(-1)), BorrowedValue::Int(-1)));
    }
    #[test]
    fn coerce_bool_yes_no_not_coerced() {
        // YAML 1.2 stance: only true/false (case variants) are bools
        let v = coerce_bool(BorrowedValue::String(Cow::Borrowed("yes")));
        assert!(matches!(v, BorrowedValue::String(s) if s == "yes"));
        let v = coerce_bool(BorrowedValue::String(Cow::Borrowed("NO")));
        assert!(matches!(v, BorrowedValue::String(s) if s == "NO"));
    }

    // coerce_str edge cases

    #[test]
    fn coerce_str_from_float() {
        let v = coerce_str(BorrowedValue::Float(3.5));
        assert!(matches!(v, BorrowedValue::String(s) if s == "3.5"));
    }
    #[test]
    fn coerce_str_identity_on_string() {
        let v = coerce_str(BorrowedValue::String(Cow::Borrowed("hi")));
        assert!(matches!(v, BorrowedValue::String(s) if s == "hi"));
    }
}
