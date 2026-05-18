//! Integration tests for scalar resolution observed through the public API.
//! Confirms YAML 1.2 Core schema scalar typing: plain integers, floats,
//! booleans, null, and the spec's Norway-problem stance (no/yes are strings).

use tmyc::{Parser, Value};

fn map_pair_value<'a>(v: &'a Value<'a>, key: &str) -> &'a Value<'a> {
    match v {
        Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, val)| match k {
                Value::String(s) if s == key => Some(val),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing key {key}")),
        _ => panic!("expected map"),
    }
}

#[test]
fn plain_int_resolves_to_uint() {
    let v = Parser::new("k: 42\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "k"), Value::UInt(42)));
}

#[test]
fn quoted_int_stays_string() {
    let v = Parser::new("k: \"42\"\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "k"), Value::String(s) if s == "42"));
}

#[test]
fn plain_negative_int() {
    let v = Parser::new("k: -42\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "k"), Value::Int(-42)));
}

#[test]
fn plain_hex_int() {
    let v = Parser::new("k: 0xff\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "k"), Value::UInt(255)));
}

#[test]
fn plain_octal_int() {
    let v = Parser::new("k: 0o755\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "k"), Value::UInt(493)));
}

#[test]
fn plain_float() {
    let v = Parser::new("k: 3.14\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "k"), Value::Float(f) if (f - 3.14).abs() < 1e-9));
}

#[test]
fn plain_inf_and_nan() {
    let v = Parser::new("a: .inf\nb: -.inf\nc: .nan\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "a"), Value::Float(f) if *f == f64::INFINITY));
    assert!(matches!(map_pair_value(&v, "b"), Value::Float(f) if *f == f64::NEG_INFINITY));
    assert!(matches!(map_pair_value(&v, "c"), Value::Float(f) if f.is_nan()));
}

#[test]
fn plain_bool_case_variants() {
    let v = Parser::new("a: true\nb: TRUE\nc: False\nd: FALSE\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "a"), Value::Bool(true)));
    assert!(matches!(map_pair_value(&v, "b"), Value::Bool(true)));
    assert!(matches!(map_pair_value(&v, "c"), Value::Bool(false)));
    assert!(matches!(map_pair_value(&v, "d"), Value::Bool(false)));
}

#[test]
fn norway_problem_yes_no_stay_strings() {
    // YAML 1.2 only recognises true/false (case variants) as bool.
    // yes/no/on/off remain strings — guarding against the famous footgun.
    let v = Parser::new("country: NO\nenabled: yes\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "country"), Value::String(s) if s == "NO"));
    assert!(matches!(map_pair_value(&v, "enabled"), Value::String(s) if s == "yes"));
}

#[test]
fn plain_null_variants() {
    let v = Parser::new("a: null\nb: ~\nc: NULL\nd:\n").parse().unwrap();
    assert!(matches!(map_pair_value(&v, "a"), Value::Null));
    assert!(matches!(map_pair_value(&v, "b"), Value::Null));
    assert!(matches!(map_pair_value(&v, "c"), Value::Null));
    assert!(matches!(map_pair_value(&v, "d"), Value::Null));
}
