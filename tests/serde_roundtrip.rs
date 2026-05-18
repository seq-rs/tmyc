//! End-to-end serde roundtrip tests across primitive, container, enum,
//! and nested-struct cases. Each test serializes a value then parses it
//! back, asserting structural equality.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Service {
    name: String,
    port: u16,
    enabled: bool,
}

#[test]
fn struct_roundtrip() {
    let svc = Service { name: "web".into(), port: 8080, enabled: true };
    let yaml = tmyc::to_string(&svc).unwrap();
    let back: Service = tmyc::from_str(&yaml).unwrap();
    assert_eq!(svc, back);
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Compose {
    services: std::collections::BTreeMap<String, ServiceEntry>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct ServiceEntry {
    image: String,
    ports: Vec<String>,
}

#[test]
fn nested_struct_with_map_and_vec() {
    let mut services = std::collections::BTreeMap::new();
    services.insert(
        "web".into(),
        ServiceEntry { image: "nginx".into(), ports: vec!["80:80".into()] },
    );
    services.insert(
        "db".into(),
        ServiceEntry { image: "postgres".into(), ports: vec!["5432:5432".into()] },
    );
    let c = Compose { services };
    let yaml = tmyc::to_string(&c).unwrap();
    let back: Compose = tmyc::from_str(&yaml).unwrap();
    assert_eq!(c, back);
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct WithOption {
    required: String,
    maybe: Option<u32>,
}

#[test]
fn option_some() {
    let v = WithOption { required: "hi".into(), maybe: Some(42) };
    let yaml = tmyc::to_string(&v).unwrap();
    let back: WithOption = tmyc::from_str(&yaml).unwrap();
    assert_eq!(v, back);
}

#[test]
fn option_none_roundtrips_as_null() {
    let v = WithOption { required: "hi".into(), maybe: None };
    let yaml = tmyc::to_string(&v).unwrap();
    let back: WithOption = tmyc::from_str(&yaml).unwrap();
    assert_eq!(v, back);
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Mode {
    Unit,
    Newtype(i32),
    Tuple(i32, String),
    Struct { x: i32, y: i32 },
}

#[test]
fn enum_unit_variant() {
    let m = Mode::Unit;
    let yaml = tmyc::to_string(&m).unwrap();
    let back: Mode = tmyc::from_str(&yaml).unwrap();
    assert_eq!(m, back);
}

#[test]
fn enum_newtype_variant() {
    let m = Mode::Newtype(42);
    let yaml = tmyc::to_string(&m).unwrap();
    let back: Mode = tmyc::from_str(&yaml).unwrap();
    assert_eq!(m, back);
}

#[test]
fn enum_tuple_variant() {
    let m = Mode::Tuple(1, "hi".into());
    let yaml = tmyc::to_string(&m).unwrap();
    let back: Mode = tmyc::from_str(&yaml).unwrap();
    assert_eq!(m, back);
}

#[test]
fn enum_struct_variant() {
    let m = Mode::Struct { x: 1, y: 2 };
    let yaml = tmyc::to_string(&m).unwrap();
    let back: Mode = tmyc::from_str(&yaml).unwrap();
    assert_eq!(m, back);
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Port(u16);

#[test]
fn newtype_struct_transparent() {
    let p = Port(8080);
    let yaml = tmyc::to_string(&p).unwrap();
    assert_eq!(yaml, "8080\n");
    let back: Port = tmyc::from_str(&yaml).unwrap();
    assert_eq!(p, back);
}

#[test]
fn vec_of_primitives() {
    let v = vec![1u32, 2, 3];
    let yaml = tmyc::to_string(&v).unwrap();
    let back: Vec<u32> = tmyc::from_str(&yaml).unwrap();
    assert_eq!(v, back);
}

#[test]
fn from_value_keeps_borrow() {
    // Demonstrates the zero-copy payoff: struct field is &str pointing into src.
    #[derive(Deserialize, Debug)]
    struct Borrowed<'a> {
        name: &'a str,
    }
    let src = "name: hello\n";
    let value = tmyc::Parser::new(src).parse().unwrap();
    let b: Borrowed<'_> = tmyc::from_value(&value).unwrap();
    assert_eq!(b.name, "hello");

    // Pointer-eq check: b.name should be inside src's byte range
    let src_start = src.as_ptr() as usize;
    let src_end = src_start + src.len();
    let name_addr = b.name.as_ptr() as usize;
    assert!(
        name_addr >= src_start && name_addr < src_end,
        "expected b.name to alias src memory"
    );
}

#[test]
fn from_str_errors_on_multi_doc() {
    let src = "---\na: 1\n---\nb: 2\n";
    let result: tmyc::Result<std::collections::BTreeMap<String, i32>> = tmyc::from_str(src);
    assert!(result.is_err());
}

#[test]
fn parse_all_multi_doc() {
    let src = "---\nkind: Pod\n---\nkind: Service\n";
    let docs = tmyc::Parser::new(src).parse_all().unwrap();
    assert_eq!(docs.len(), 2);
}
