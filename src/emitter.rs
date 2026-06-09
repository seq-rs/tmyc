use crate::Result;
use crate::BorrowedValue;
use crate::patterns::has_ctrl_chars;
use crate::patterns::has_newline;
use crate::patterns::needs_quotes;
use std::fmt::Write;

pub fn emit(v: &BorrowedValue<'_>) -> Result<String> {
    let mut out = String::new();
    emit_node(v, 0, &mut out)?;
    if out.starts_with('\n') {
        out.remove(0);
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn emit_node(v: &BorrowedValue<'_>, indent: usize, out: &mut String) -> Result<()> {
    match v {
        BorrowedValue::Null => emit_null(out),
        BorrowedValue::Bool(b) => emit_bool(*b, out),
        BorrowedValue::Int(n) => emit_int(n, out),
        BorrowedValue::UInt(n) => emit_uint(n, out),
        BorrowedValue::Float(f) => emit_float(f, out),
        BorrowedValue::String(s) => emit_scalar(s, indent, out),
        BorrowedValue::Tagged(tag, v) => {
            out.push_str(tag);
            if is_inline(v) {
                out.push(' ');
            }
            emit_node(v, indent, out)?;
        }
        BorrowedValue::Seq(items) => emit_block_seq(items, indent, out)?,
        BorrowedValue::Map(pairs) => emit_block_map(pairs, indent, out)?,
    }
    Ok(())
}

fn emit_null(out: &mut String) {
    out.push_str("null")
}

fn emit_bool(b: bool, out: &mut String) {
    out.push_str(if b { "true" } else { "false" });
}

fn emit_int(n: &i64, out: &mut String) {
    write!(out, "{n}").unwrap();
}

fn emit_uint(n: &u64, out: &mut String) {
    write!(out, "{n}").unwrap();
}

fn emit_float(f: &f64, out: &mut String) {
    if f.is_nan() {
        out.push_str(".nan");
        return;
    }

    if f.is_infinite() {
        out.push_str(if f.is_sign_positive() {
            ".inf"
        } else {
            "-.inf"
        });
        return;
    }

    let s = format!("{}", f);
    out.push_str(&s);
    if !s.contains('.') && !s.contains('e') {
        out.push_str(".0");
    }
}

fn emit_scalar(s: &str, indent: usize, out: &mut String) {
    if !s.is_empty() && has_newline(s) && !has_ctrl_chars(s) {
        emit_block_scalar(s, indent, out);
    } else if needs_quotes(s) {
        emit_quoted_scalar(s, out);
    } else {
        out.push_str(s);
    }
}

fn emit_block_scalar(s: &str, indent: usize, out: &mut String) {
    out.push('|');
    if !s.ends_with('\n') {
        out.push('-');
    }
    let child = " ".repeat(indent + 2);
    for line in s.split('\n') {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&child);
            out.push_str(line);
        }
    }
}

fn emit_quoted_scalar(s: &str, out: &mut String) {
    if !has_ctrl_chars(s) && !s.contains('\'') {
        out.push('\'');
        out.push_str(s);
        out.push('\'');
    } else {
        emit_double_quoted(s, out);
    }
}

fn emit_double_quoted(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                write!(out, "\\x{:02x}", c as u32).unwrap()
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn emit_block_seq(items: &[BorrowedValue<'_>], indent: usize, out: &mut String) -> Result<()> {
    if items.is_empty() {
        out.push_str("[]");
        return Ok(());
    }
    for item in items {
        out.push('\n');
        push_indent(out, indent);
        push_block_prefix(out);
        match item {
            BorrowedValue::Map(pairs) if !pairs.is_empty() => {
                emit_kv(&pairs[0], indent + 2, out)?;
                for kv in &pairs[1..] {
                    out.push('\n');
                    push_indent(out, indent + 2);
                    emit_kv(kv, indent + 2, out)?;
                }
            }
            _ => emit_node(item, indent + 2, out)?,
        }
    }
    Ok(())
}

fn emit_block_map(pairs: &[(BorrowedValue<'_>, BorrowedValue<'_>)], indent: usize, out: &mut String) -> Result<()> {
    if pairs.is_empty() {
        out.push('{');
        out.push('}');
        return Ok(());
    }
    for kv in pairs {
        out.push('\n');
        push_indent(out, indent);
        emit_kv(kv, indent, out)?;
    }
    Ok(())
}

fn emit_kv(kv: &(BorrowedValue<'_>, BorrowedValue<'_>), indent: usize, out: &mut String) -> Result<()> {
    let (k, v) = kv;
    emit_node(k, indent, out)?;
    out.push(':');
    if is_inline(v) {
        out.push(' ');
    }
    emit_node(v, indent + 2, out)?;
    Ok(())
}

fn push_indent(out: &mut String, indent: usize) {
    out.push_str(&" ".repeat(indent))
}

fn push_block_prefix(out: &mut String) {
    out.push('-');
    out.push(' ');
}

fn is_inline(v: &BorrowedValue<'_>) -> bool {
    match v {
        BorrowedValue::Seq(items) if !items.is_empty() => false,
        BorrowedValue::Map(items) if !items.is_empty() => false,
        BorrowedValue::Tagged(_, value) => is_inline(value),
        _ => true,
    }
}
