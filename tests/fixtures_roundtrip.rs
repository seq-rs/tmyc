//! Parse → emit → parse roundtrip tests against real-world-shaped YAML.
//!
//! The "acceptance bar" from `tmyc-76k`: confirm that the data survives a
//! full roundtrip through the public API. Presentation details may shift
//! (folded blocks become literal, anchors get expanded), but the
//! resulting [`tmyc::BorrowedValue`] must be structurally equal between the
//! first and second parse.

use std::fs;

use tmyc::{Parser, BorrowedValue};

const FIXTURES: &str = "tests/fixtures";

/// Roundtrip a single-document fixture and assert structural equality.
fn assert_roundtrips(path: &str) {
    let src = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"));
    let first = Parser::new(&src).parse().expect("first parse");
    let emitted = tmyc::to_string(&first).expect("emit");
    let second = Parser::new(&emitted).parse().expect("reparse");
    assert_eq!(
        first, second,
        "fixture {path} did not survive parse→emit→parse roundtrip\n\
         emitted form was:\n{emitted}"
    );
}

#[test]
fn k8s_pod_roundtrips() {
    assert_roundtrips(&format!("{FIXTURES}/k8s_pod.yaml"));
}

#[test]
fn compose_roundtrips_with_merge_keys_resolved() {
    // After first parse, merge keys (<<: *defaults) are resolved into flat
    // maps. The emitted form has no anchors or merge keys — so the second
    // parse produces an equivalent flat structure. This is the expected
    // pragmatic behaviour, documented in CLAUDE.md.
    assert_roundtrips(&format!("{FIXTURES}/compose.yaml"));
}

#[test]
fn sops_secret_roundtrips() {
    // Block scalars: literal (cert) preserves newlines, folded (description)
    // gets joined into a flat string at parse time. On second parse, both
    // are simply parsed as strings — equality holds.
    assert_roundtrips(&format!("{FIXTURES}/sops_secret.yaml"));
}

#[test]
fn kubectl_stream_per_doc_roundtrips() {
    let src = fs::read_to_string(format!("{FIXTURES}/kubectl_stream.yaml")).unwrap();
    let docs = Parser::new(&src).parse_all().expect("parse_all");
    assert_eq!(docs.len(), 2, "expected 2 docs in kubectl_stream");

    for (i, doc) in docs.iter().enumerate() {
        let emitted = tmyc::to_string(doc).expect("emit");
        let reparsed = Parser::new(&emitted).parse().expect("reparse");
        assert_eq!(
            doc, &reparsed,
            "doc {i} in kubectl_stream did not roundtrip\nemitted:\n{emitted}"
        );
    }
}

#[test]
fn value_construct_emit_parse_matches() {
    // Construct a BorrowedValue by hand, emit, parse — confirm BorrowedValue: Serialize
    // produces the same shape that the parser would build from equivalent YAML.
    use std::borrow::Cow;
    let constructed = BorrowedValue::Map(vec![
        (
            BorrowedValue::String(Cow::Borrowed("name")),
            BorrowedValue::String(Cow::Borrowed("test")),
        ),
        (BorrowedValue::String(Cow::Borrowed("count")), BorrowedValue::UInt(42)),
    ]);
    let emitted = tmyc::to_string(&constructed).unwrap();
    let reparsed = Parser::new(&emitted).parse().unwrap();
    assert_eq!(constructed, reparsed);
}

#[test]
fn empty_doc_roundtrip() {
    let first = Parser::new("").parse().unwrap();
    let emitted = tmyc::to_string(&first).unwrap();
    let second = Parser::new(&emitted).parse().unwrap();
    assert_eq!(first, second);
    assert!(matches!(first, BorrowedValue::Null));
}
