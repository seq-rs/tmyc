use std::borrow::Cow;

use crate::BorrowedValue;

pub(crate) fn seems_null(s: &str) -> bool {
    matches!(s, "" | "~" | "null" | "Null" | "NULL")
}

pub(crate) fn seems_bool(s: &str) -> bool {
    s == "true" || s == "false"
}

pub(crate) fn seems_int(s: &str) -> bool {
    s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok()
}

pub(crate) fn parse_yaml_int(s: &str) -> Option<BorrowedValue<'static>> {
    if let Some(hex) = s.strip_prefix("0x") {
        return i64::try_from(u64::from_str_radix(hex, 16).ok()?)
            .ok()
            .map(BorrowedValue::Int);
    }
    if let Some(oct) = s.strip_prefix("0o") {
        return i64::try_from(u64::from_str_radix(oct, 8).ok()?)
            .ok()
            .map(BorrowedValue::Int);
    }
    let body = s.strip_prefix('+').unwrap_or(s);
    body.parse::<i64>().ok().map(BorrowedValue::Int)
}

pub(crate) fn seems_float(s: &str) -> bool {
    matches!(s, ".inf" | "-.inf" | "+.inf" | ".nan")
        || (s.parse::<f64>().is_ok() && s.bytes().any(|b| b == b'.' || b == b'e' || b == b'E'))
}

pub(crate) fn parse_yaml_float(s: &str) -> Option<f64> {
    match s {
        ".nan" | ".NaN" | ".NAN" => return Some(f64::NAN),
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => return Some(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    s.parse::<f64>().ok()
}

pub(crate) fn seems_scalar_typed(s: &str) -> bool {
    seems_null(s) || seems_bool(s) || seems_int(s) || seems_float(s)
}

pub(crate) fn resolve_scalar<'a>(s: Cow<'a, str>) -> BorrowedValue<'a> {
    use BorrowedValue::*;
    match s.as_ref() {
        "" | "~" | "null" | "Null" | "NULL" => return Null,
        "true" | "True" | "TRUE" => return Bool(true),
        "false" | "False" | "FALSE" => return Bool(false),
        _ => {}
    }

    if let Some(v) = parse_yaml_int(&s) {
        return v;
    }

    if s.contains(['.', 'e', 'E'])
        && let Some(n) = parse_yaml_float(&s)
    {
        return Float(n);
    }

    BorrowedValue::String(s)
}

pub(crate) fn has_ctrl_chars(s: &str) -> bool {
    s.bytes()
        .any(|b| matches!(b, 0..=0x08 | 0x0b..=0x1f | 0x7f))
}

pub(crate) fn has_newline(s: &str) -> bool {
    s.bytes().any(|b| b == b'\n')
}

pub(crate) fn has_yaml_special(s: &str) -> bool {
    // chars that mid-string can confuse a plain-scalar parser
    s.bytes().any(|b| matches!(b, b':' | b'#'))
    // technically only ':' and ' #' are ambiguous, but no chance taking
}

pub(crate) fn starts_with_indicator(s: &str) -> bool {
    matches!(
        s.as_bytes().first(),
        Some(
            b'-' | b'?'
                | b'!'
                | b'&'
                | b'*'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'|'
                | b'>'
                | b'\''
                | b'"'
                | b'%'
                | b'@'
                | b'`'
                | b','
                | b'#'
        )
    )
}

pub(crate) fn has_leading_or_trailing_space(s: &str) -> bool {
    matches!(s.as_bytes().first(), Some(b' ' | b'\t'))
        || matches!(s.as_bytes().last(), Some(b' ' | b'\t'))
}

pub(crate) fn needs_quotes(s: &str) -> bool {
    s.is_empty()
        || seems_scalar_typed(s)
        || has_ctrl_chars(s)
        || has_newline(s)
        || has_yaml_special(s)
        || has_leading_or_trailing_space(s)
        || starts_with_indicator(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_null() {
        assert!(matches!(resolve_scalar("null".into()), BorrowedValue::Null));
    }
    #[test]
    fn resolves_tilde() {
        assert!(matches!(resolve_scalar("~".into()), BorrowedValue::Null));
    }
    #[test]
    fn resolves_true() {
        assert!(matches!(
            resolve_scalar("true".into()),
            BorrowedValue::Bool(true)
        ));
    }
    #[test]
    fn resolves_int() {
        assert!(matches!(
            resolve_scalar("42".into()),
            BorrowedValue::Int(42)
        ));
    }
    #[test]
    fn resolves_negative_int() {
        assert!(matches!(
            resolve_scalar("-42".into()),
            BorrowedValue::Int(-42)
        ));
    }
    #[test]
    fn resolves_big_uint_as_string() {
        assert!(matches!(
            resolve_scalar("18446744073709551610".into()),
            BorrowedValue::String(_)
        ));
    }
    #[test]
    fn resolves_float() {
        assert!(matches!(
            resolve_scalar("1.5".into()),
            BorrowedValue::Float(_)
        ));
    }
    #[test]
    fn resolves_inf() {
        let v = resolve_scalar(".inf".into());
        assert!(matches!(v, BorrowedValue::Float(f) if f == f64::INFINITY));
    }
    #[test]
    fn resolves_neg_inf() {
        let v = resolve_scalar("-.inf".into());
        assert!(matches!(v, BorrowedValue::Float(f) if f == f64::NEG_INFINITY));
    }
    #[test]
    fn resolves_nan() {
        let v = resolve_scalar(".nan".into());
        assert!(matches!(v, BorrowedValue::Float(f) if f.is_nan()));
    }
    #[test]
    fn resolves_int_text() {
        let v = resolve_scalar("42".into());
        assert!(!matches!(v, BorrowedValue::Float(_)));
    }
    #[test]
    fn falls_back_to_string() {
        assert!(matches!(
            resolve_scalar("hello".into()),
            BorrowedValue::String(_)
        ));
    }
    #[test]
    fn preserves_borrow_on_string() {
        let src = "hello";
        let cow: Cow<'_, str> = Cow::Borrowed(src);
        let v = resolve_scalar(cow);
        if let BorrowedValue::String(Cow::Borrowed(s)) = v {
            assert!(std::ptr::eq(s, src));
        } else {
            panic!("expected borrowed string");
        }
    }

    // parse_yaml_int

    #[test]
    fn yaml_int_decimal() {
        assert!(matches!(
            parse_yaml_int("42"),
            Some(BorrowedValue::Int(42))
        ));
    }
    #[test]
    fn yaml_int_negative() {
        assert!(matches!(
            parse_yaml_int("-42"),
            Some(BorrowedValue::Int(-42))
        ));
    }
    #[test]
    fn yaml_int_plus_prefix() {
        assert!(matches!(
            parse_yaml_int("+42"),
            Some(BorrowedValue::Int(42))
        ));
    }
    #[test]
    fn yaml_int_hex() {
        assert!(matches!(
            parse_yaml_int("0xff"),
            Some(BorrowedValue::Int(255))
        ));
    }
    #[test]
    fn yaml_int_hex_mixed_case() {
        assert!(matches!(
            parse_yaml_int("0xAbCd"),
            Some(BorrowedValue::Int(43981))
        ));
    }
    #[test]
    fn yaml_int_octal() {
        assert!(matches!(
            parse_yaml_int("0o755"),
            Some(BorrowedValue::Int(493))
        ));
    }
    #[test]
    fn yaml_int_hex_no_sign() {
        // spec: hex/octal forbid sign — fall back to None
        assert!(parse_yaml_int("+0xff").is_none());
        assert!(parse_yaml_int("-0xff").is_none());
    }
    #[test]
    fn yaml_int_empty_radix() {
        assert!(parse_yaml_int("0x").is_none());
        assert!(parse_yaml_int("0o").is_none());
    }
    #[test]
    fn yaml_int_i64_min() {
        assert!(matches!(
            parse_yaml_int("-9223372036854775808"),
            Some(BorrowedValue::Int(i64::MIN))
        ));
    }
    #[test]
    fn yaml_int_overflow_falls_back() {
        assert!(parse_yaml_int("-99999999999999999999").is_none());
    }
    #[test]
    fn yaml_int_garbage() {
        assert!(parse_yaml_int("hello").is_none());
        assert!(parse_yaml_int("3.14").is_none());
    }

    // parse_yaml_float

    #[test]
    fn yaml_float_decimal() {
        assert!(matches!(parse_yaml_float("3.1"), Some(f) if (f - 3.1).abs() < 1e-9));
    }
    #[test]
    fn yaml_float_scientific() {
        assert!(matches!(parse_yaml_float("1e5"), Some(f) if f == 100_000.0));
    }
    #[test]
    fn yaml_float_inf_variants() {
        for s in [".inf", ".Inf", ".INF", "+.inf", "+.Inf", "+.INF"] {
            assert_eq!(parse_yaml_float(s), Some(f64::INFINITY), "{s}");
        }
    }
    #[test]
    fn yaml_float_neg_inf_variants() {
        for s in ["-.inf", "-.Inf", "-.INF"] {
            assert_eq!(parse_yaml_float(s), Some(f64::NEG_INFINITY), "{s}");
        }
    }
    #[test]
    fn yaml_float_nan_variants() {
        for s in [".nan", ".NaN", ".NAN"] {
            assert!(matches!(parse_yaml_float(s), Some(f) if f.is_nan()), "{s}");
        }
    }
    #[test]
    fn yaml_float_accepts_int_text() {
        // helper is permissive — gating happens in resolve_scalar
        assert!(matches!(parse_yaml_float("42"), Some(f) if f == 42.0));
    }
    #[test]
    fn yaml_float_garbage() {
        assert!(parse_yaml_float("hello").is_none());
        assert!(parse_yaml_float(".inferno").is_none());
    }

    // resolve_scalar — new cases routed through helpers

    #[test]
    fn resolves_hex() {
        assert!(matches!(
            resolve_scalar("0xff".into()),
            BorrowedValue::Int(255)
        ));
    }
    #[test]
    fn resolves_octal() {
        assert!(matches!(
            resolve_scalar("0o17".into()),
            BorrowedValue::Int(15)
        ));
    }
    #[test]
    fn resolves_plus_int() {
        assert!(matches!(
            resolve_scalar("+42".into()),
            BorrowedValue::Int(42)
        ));
    }
}
