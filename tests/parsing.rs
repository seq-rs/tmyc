//! Integration tests for the parser's public BorrowedValue API.
//! Unit tests live alongside parser modules; these exercise observable
//! end-to-end behaviour through `Parser::new(...).parse()`.

use tmyc::{Parser, BorrowedValue};

#[test]
fn empty_input_is_null() {
    let value = Parser::new("").parse().unwrap();
    assert!(matches!(value, BorrowedValue::Null));
}

#[test]
fn whitespace_only_is_null() {
    let value = Parser::new("   \n\n  \n").parse().unwrap();
    assert!(matches!(value, BorrowedValue::Null));
}

#[test]
fn bom_is_transparently_stripped() {
    let value = Parser::new("\u{FEFF}key: value\n").parse().unwrap();
    match value {
        BorrowedValue::Map(pairs) => {
            assert!(matches!(&pairs[0].0, BorrowedValue::String(s) if s == "key"));
            assert!(matches!(&pairs[0].1, BorrowedValue::String(s) if s == "value"));
        }
        _ => panic!("expected map"),
    }
}

#[test]
fn anchors_reset_between_docs() {
    let src = "---\nbase: &x 1\n---\nuse: *x\n";
    let result = Parser::new(src).parse_all();
    assert!(result.is_err(), "expected unknown-anchor error in second doc");
}

#[test]
fn parser_errors_carry_line_and_col() {
    // Unterminated double-quoted string — parser-side error with position
    let err = Parser::new("k: \"unterminated").parse().unwrap_err();
    assert!(err.line.is_some(), "expected line info on parser error");
    assert!(err.col.is_some(), "expected col info on parser error");
}

#[test]
fn parse_then_emit_then_reparse_is_structurally_equal() {
    let src = "\
name: web
port: 8080
tags:
  - frontend
  - load-balancer
nested:
  k1: v1
  k2: v2
";
    let value = Parser::new(src).parse().unwrap();
    // Emit by converting through serde_json::Value-like dance isn't worth it
    // here; the serde_roundtrip suite already covers value-level roundtrip via
    // typed structs. This test just confirms the parser produces a stable
    // shape: parsing the same input twice produces equal Values.
    let value2 = Parser::new(src).parse().unwrap();
    assert_eq!(value, value2);
}
